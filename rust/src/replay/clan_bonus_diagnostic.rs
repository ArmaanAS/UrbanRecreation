//! Replay preparation and execution for the explicit clan-bonus diagnostic projection.
//!
//! Cold preparation retains rich source dispositions and produces compact engine plans once.
//! Execute means admitted by the projection; Stop Bonus or cancellation may still suppress it.

use super::execute::{
    round_input, BaseRulesReplay, BaseRulesReplayRoundContext, ReplayValidationError,
};
use super::model::{ReplayCaseV1, ReplayRound};
use crate::catalog::{CardCatalog, CardKey};
use crate::effect_registry::{
    AffectedSideV1, AttributeActionV1, AttributeAffectedV1, CombatStatV1, CompiledEffectV1,
    EffectLookupError, EffectRegistryV1, MagnitudeMultiplierV1, SourceFingerprintFnv1a64,
    SpecialActionV1, StatOperationV1, SupportedEffectV1, UnsupportedReasonV1,
};
use crate::engine::{
    BaseRulesPosition, BaseRulesRoundInput, BaseRulesRoundReport, ByPlayer, ClanBonusDiagnostic,
    ClanBonusDiagnosticError, ClanBonusDiagnosticMatchSpecV1, DiagnosticAffectedSideV1,
    DiagnosticCardPlanV1, DiagnosticCombatEffectV1, DiagnosticCombatStatV1,
    DiagnosticEffectSourceV1, DiagnosticMagnitudeV1, DiagnosticPlanErrorV1, DiagnosticSourcePlanV1,
    DiagnosticStatOperationV1, PlayerId, HAND_SIZE,
};
use std::error::Error;
use std::fmt;

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

/// Registry and policy provenance for one replay-prepared diagnostic projection.
///
/// The FNV-1a value is a deterministic source-byte change detector, not a cryptographic hash.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ClanBonusDiagnosticProvenanceV1 {
    pub model: DiagnosticReplayModelV1,
    pub projection: ClanBonusDiagnosticProjectionV1,
    pub effect_registry_schema_version: u16,
    pub effect_registry_source_fingerprint_fnv1a64: SourceFingerprintFnv1a64,
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
    pub provenance: ClanBonusDiagnosticProvenanceV1,
    pub round: BaseRulesRoundReport,
    pub selected: ByPlayer<ClanBonusDiagnosticSelectedCardReportV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClanBonusDiagnosticReplayReportV1 {
    pub battle_id: u64,
    pub model: DiagnosticReplayModelV1,
    pub provenance: ClanBonusDiagnosticProvenanceV1,
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
        selected: ByPlayer<ClanBonusDiagnosticSelectedCardReportV1>,
        source: ClanBonusDiagnosticError,
    },
    Mismatch {
        context: BaseRulesReplayRoundContext,
        selected: ByPlayer<ClanBonusDiagnosticSelectedCardReportV1>,
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
            Self::Engine {
                context,
                selected,
                source,
            } => {
                write!(
                    formatter,
                    "{model} engine error at {context}: {source}; selected={selected:?}"
                )
            }
            Self::Mismatch {
                context,
                selected,
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "{model} mismatch at {context}: {field}: expected {expected}, actual {actual}; selected={selected:?}"
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
    provenance: ClanBonusDiagnosticProvenanceV1,
}

struct PreparedDiagnosticSourceV1 {
    disposition: DiagnosticProjectionDispositionV1,
    compact_plan: DiagnosticSourcePlanV1,
}

struct PreparedDiagnosticCardV1 {
    metadata: DiagnosticCardPreparationV1,
    compact_plan: DiagnosticCardPlanV1,
}

struct PreparedDiagnosticCardsV1 {
    metadata: ByPlayer<[DiagnosticCardPreparationV1; HAND_SIZE]>,
    compact_plans: ByPlayer<[DiagnosticCardPlanV1; HAND_SIZE]>,
}

impl ClanBonusDiagnosticReplayV1 {
    pub fn new(
        replay: ReplayCaseV1,
        catalog: &CardCatalog,
        registry: &EffectRegistryV1,
        projection: ClanBonusDiagnosticProjectionV1,
    ) -> Result<Self, ClanBonusDiagnosticPreparationErrorV1> {
        let base = BaseRulesReplay::new(replay, catalog)
            .map_err(ClanBonusDiagnosticPreparationErrorV1::Replay)?;
        let battle_id = base.battle_id();
        let prepared = prepare_diagnostic_cards(base.replay(), registry, battle_id)?;
        let match_spec = ClanBonusDiagnosticMatchSpecV1 {
            base_rules: base.match_spec().clone(),
            cards: prepared.compact_plans,
        };
        ClanBonusDiagnostic::new(match_spec.clone())
            .map_err(ClanBonusDiagnosticPreparationErrorV1::EnginePlan)?;
        let provenance = ClanBonusDiagnosticProvenanceV1 {
            model: DiagnosticReplayModelV1::ClanBonusDiagnosticV1,
            projection,
            effect_registry_schema_version: registry.schema_version(),
            effect_registry_source_fingerprint_fnv1a64: registry.source_fingerprint_fnv1a64(),
        };
        Ok(Self {
            base,
            match_spec,
            cards: prepared.metadata,
            provenance,
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

    pub const fn preparation_provenance(&self) -> ClanBonusDiagnosticProvenanceV1 {
        self.provenance
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
            let (report, _) =
                game.make(input)
                    .map_err(|source| ClanBonusDiagnosticReplayErrorV1::Engine {
                        context,
                        selected: selected.clone(),
                        source,
                    })?;
            assert_diagnostic_round(replay_round, &report, context, &selected)?;
            reports.push(ClanBonusDiagnosticRoundReportV1 {
                model: DiagnosticReplayModelV1::ClanBonusDiagnosticV1,
                provenance: self.provenance,
                round: report,
                selected,
            });
        }
        Ok(ClanBonusDiagnosticReplayReportV1 {
            battle_id: self.battle_id(),
            model: DiagnosticReplayModelV1::ClanBonusDiagnosticV1,
            provenance: self.provenance,
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
) -> Result<PreparedDiagnosticCardsV1, ClanBonusDiagnosticPreparationErrorV1> {
    let mut prepared: ByPlayer<[Option<PreparedDiagnosticCardV1>; HAND_SIZE]> =
        ByPlayer::new([const { None }; HAND_SIZE], [const { None }; HAND_SIZE]);
    for player in PlayerId::ALL {
        for slot in 0..HAND_SIZE {
            let card = &replay.players[player.index()].hand[slot];
            let source_bonus_support_count = source_bonus_support_count(
                &replay.players[player.index()].hand,
                card.source_bonus.as_ref().map(|modifier| modifier.id),
            );
            let ability = prepare_diagnostic_source(
                registry,
                battle_id,
                player,
                slot as u8,
                DiagnosticEffectSourceV1::Ability,
                card.source_ability.as_ref(),
            )?;
            let bonus = prepare_diagnostic_source(
                registry,
                battle_id,
                player,
                slot as u8,
                DiagnosticEffectSourceV1::Bonus,
                card.source_bonus.as_ref(),
            )?;
            prepared[player][slot] = Some(PreparedDiagnosticCardV1 {
                metadata: DiagnosticCardPreparationV1 {
                    key: card.key,
                    source_bonus_support_count,
                    ability: ability.disposition,
                    bonus: bonus.disposition,
                },
                compact_plan: DiagnosticCardPlanV1 {
                    key: card.key,
                    source_bonus_support_count,
                    ability: ability.compact_plan,
                    bonus: bonus.compact_plan,
                },
            });
        }
    }
    let ByPlayer([p1, p2]) = prepared;
    let prepared = ByPlayer::new(
        p1.map(|card| card.expect("all four P1 cards were prepared")),
        p2.map(|card| card.expect("all four P2 cards were prepared")),
    );
    Ok(PreparedDiagnosticCardsV1 {
        metadata: ByPlayer::new(
            std::array::from_fn(|slot| prepared[PlayerId::P1][slot].metadata.clone()),
            std::array::from_fn(|slot| prepared[PlayerId::P2][slot].metadata.clone()),
        ),
        compact_plans: ByPlayer::new(
            std::array::from_fn(|slot| prepared[PlayerId::P1][slot].compact_plan),
            std::array::from_fn(|slot| prepared[PlayerId::P2][slot].compact_plan),
        ),
    })
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
) -> Result<PreparedDiagnosticSourceV1, ClanBonusDiagnosticPreparationErrorV1> {
    let Some(source) = source else {
        return Ok(PreparedDiagnosticSourceV1 {
            disposition: DiagnosticProjectionDispositionV1::Absent,
            compact_plan: DiagnosticSourcePlanV1::Absent,
        });
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
            // Preserve this legacy projection's pre-SOA behavior. It has no ability-
            // liveness model, so exact SOA remains an ordinary disabled card-local source;
            // the current combat-stat projection owns executable Stop Opp. Ability. The
            // same holds for Protection, which needs both a liveness model and the
            // refusal of an opposing reduction: here it stays a disabled card-local
            // source, as it was before the registry learned to compile it.
            if matches!(
                effect,
                SupportedEffectV1::StopOpponentAbility
                    | SupportedEffectV1::ProtectOwnCombatStat { .. }
                    | SupportedEffectV1::ProtectOwnAbility
                    | SupportedEffectV1::ProtectOwnBonus
                    | SupportedEffectV1::CopyOpponentPrintedCombatStat { .. }
            ) {
                let reason = if source_kind == DiagnosticEffectSourceV1::Ability {
                    DiagnosticDisabledReasonV1::OrdinaryAbility {
                        registry_reasons: Box::new([]),
                    }
                } else {
                    DiagnosticDisabledReasonV1::OutOfSliceBonus {
                        registry_reasons: Box::new([]),
                    }
                };
                return Ok(PreparedDiagnosticSourceV1 {
                    disposition: DiagnosticProjectionDispositionV1::Disabled { identity, reason },
                    compact_plan: DiagnosticSourcePlanV1::Disabled {
                        source_id: source.id,
                    },
                });
            }
            let execute = source_kind == DiagnosticEffectSourceV1::Bonus
                || matches!(
                    effect,
                    SupportedEffectV1::StopOpponentBonus
                        | SupportedEffectV1::CancelOpponentCombatStatModifiers { .. }
                );
            if execute {
                let compact_plan = compact_effect(*effect).ok_or(
                    ClanBonusDiagnosticPreparationErrorV1::UnsupportedCompiledShape {
                        battle_id,
                        player,
                        hand_slot,
                        source_kind,
                        effect_id: source.id,
                    },
                )?;
                Ok(PreparedDiagnosticSourceV1 {
                    disposition: DiagnosticProjectionDispositionV1::Execute {
                        identity,
                        effect: *effect,
                    },
                    compact_plan: DiagnosticSourcePlanV1::Execute {
                        source_id: source.id,
                        effect: compact_plan,
                    },
                })
            } else {
                Ok(PreparedDiagnosticSourceV1 {
                    disposition: DiagnosticProjectionDispositionV1::Disabled {
                        identity,
                        reason: DiagnosticDisabledReasonV1::OrdinaryAbility {
                            registry_reasons: Box::new([]),
                        },
                    },
                    compact_plan: DiagnosticSourcePlanV1::Disabled {
                        source_id: source.id,
                    },
                })
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
            let compact_plan = if attempted_control {
                DiagnosticSourcePlanV1::RejectIfSelected {
                    source_id: source.id,
                }
            } else {
                DiagnosticSourcePlanV1::Disabled {
                    source_id: source.id,
                }
            };
            Ok(PreparedDiagnosticSourceV1 {
                disposition: DiagnosticProjectionDispositionV1::Disabled { identity, reason },
                compact_plan,
            })
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
                    | AttributeAffectedV1::PowerAndAttack
                    | AttributeAffectedV1::PowerAndDamage
            ))
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
                MagnitudeMultiplierV1::Growth
                | MagnitudeMultiplierV1::Degrowth
                | MagnitudeMultiplierV1::OpponentStars
                | MagnitudeMultiplierV1::OpponentDamage => return None,
            },
        }),
        SupportedEffectV1::StopOpponentAbility => None,
        SupportedEffectV1::StopOpponentBonus => Some(DiagnosticCombatEffectV1::StopOpponentBonus),
        // Protection belongs to the combat-stat projection and its gate. This older
        // clan-bonus projection has never admitted a control channel of its own.
        SupportedEffectV1::ProtectOwnCombatStat { .. }
        | SupportedEffectV1::ProtectOwnAbility
        | SupportedEffectV1::ProtectOwnBonus
        | SupportedEffectV1::CopyOpponentPrintedCombatStat { .. } => None,
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

fn assert_diagnostic_round(
    expected: &ReplayRound,
    actual: &BaseRulesRoundReport,
    context: BaseRulesReplayRoundContext,
    selected: &ByPlayer<ClanBonusDiagnosticSelectedCardReportV1>,
) -> Result<(), ClanBonusDiagnosticReplayErrorV1> {
    for player in PlayerId::ALL {
        let expected_player = expected.expected_player_states[player.index()];
        compare_diagnostic_field(
            context,
            selected,
            format!("players.{player:?}.life"),
            expected_player.life,
            actual.players[player].life,
        )?;
        compare_diagnostic_field(
            context,
            selected,
            format!("players.{player:?}.pillz"),
            expected_player.pillz,
            actual.players[player].pillz,
        )?;
        if let Some(expected_card) = expected.expected_card_results[player.index()] {
            let actual_card = actual.cards[player];
            compare_diagnostic_field(
                context,
                selected,
                format!("cards.{player:?}.power"),
                expected_card.power,
                actual_card.power,
            )?;
            compare_diagnostic_field(
                context,
                selected,
                format!("cards.{player:?}.damage"),
                expected_card.damage,
                actual_card.damage,
            )?;
            compare_diagnostic_field(
                context,
                selected,
                format!("cards.{player:?}.attack"),
                expected_card.attack,
                actual_card.attack,
            )?;
            compare_diagnostic_field(
                context,
                selected,
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
    selected: &ByPlayer<ClanBonusDiagnosticSelectedCardReportV1>,
    field: String,
    expected: T,
    actual: T,
) -> Result<(), ClanBonusDiagnosticReplayErrorV1> {
    if expected == actual {
        Ok(())
    } else {
        Err(ClanBonusDiagnosticReplayErrorV1::Mismatch {
            context,
            selected: selected.clone(),
            field,
            expected: format!("{expected:?}"),
            actual: format!("{actual:?}"),
        })
    }
}
