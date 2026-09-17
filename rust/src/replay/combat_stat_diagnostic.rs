//! Replay preparation and execution for the explicit combat-stat diagnostic projection.
//!
//! Cold preparation retains rich source dispositions and produces compact engine plans once.
//! Execute means admitted by the projection; Stop Bonus or cancellation may still suppress it.

use super::execute::{
    round_input, BaseRulesReplay, BaseRulesReplayRoundContext, ReplayValidationError,
};
use super::model::{ReplayCaseV1, ReplayRound};
use crate::catalog::{CardCatalog, CardKey};
use crate::effect_registry::{
    AttributeActionV1, AttributeAffectedV1, EffectDefinitionV1, EffectLookupError,
    EffectRegistryV1, SourceFingerprintFnv1a64, SpecialActionV1, StructuredEffectV1,
    SupportedEffectV1, UnsupportedReasonV1,
};
use crate::engine::combat_stat_compiler::{
    classify_anita_courage_damage_to_life, classify_argos_defeat_capped_pillz,
    classify_combat_stat_effect, classify_defeat_life, classify_defeat_recover_pillz,
    classify_equalizer_opponent_life_on_victory, classify_komboka_victory_pillz_and_life,
    classify_reanimate_life, classify_victory_life, classify_victory_opponent_life,
    classify_victory_or_defeat_life, classify_victory_or_defeat_pillz, compact_effect,
    has_defeat_life_shape, has_reanimate_life_shape, has_victory_life_shape,
    VictoryOrDefeatLifeEffectV1, COMBAT_STAT_COMPILER_POLICY_SEMANTIC_REVISION_V1,
};
use crate::engine::{
    derive_effective_catalog_hand, BaseRulesPosition, BaseRulesRoundInput, BaseRulesRoundReport,
    ByPlayer, CombatStatCardPlanV1, CombatStatDiagnosticErrorV1, CombatStatDiagnosticMatchSpecV1,
    CombatStatDiagnosticV1, CombatStatEffectSourceV1, CombatStatEffectV1, CombatStatMagnitudeV1,
    CombatStatPlanErrorV1, CombatStatPostRoundEffectV1, CombatStatPredicateV1,
    CombatStatSourcePlanV1, PlayerId, HAND_SIZE,
};
use std::error::Error;
use std::fmt;

/// Explicit authorization for the deliberately incomplete replay projection.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CombatStatDiagnosticProjectionV1 {
    DisableDeferredAndOutOfSliceCardLocalEffects,
}

pub const COMBAT_STAT_DIAGNOSTIC_COMPILER_POLICY_SEMANTIC_REVISION_V1: u16 =
    COMBAT_STAT_COMPILER_POLICY_SEMANTIC_REVISION_V1;

const LOBO_REANIMATE_REGISTRY_ID: u32 = 4951;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CombatStatReplayModelV1 {
    CombatStatDiagnosticV1,
}

impl fmt::Display for CombatStatReplayModelV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("combat-stat-diagnostic-v1")
    }
}

/// Registry and policy provenance for one replay-prepared combat-stat diagnostic projection.
///
/// The FNV-1a value is a deterministic source-byte change detector, not a cryptographic hash.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CombatStatDiagnosticProvenanceV1 {
    pub model: CombatStatReplayModelV1,
    pub projection: CombatStatDiagnosticProjectionV1,
    pub effect_registry_schema_version: u16,
    pub effect_registry_source_fingerprint_fnv1a64: SourceFingerprintFnv1a64,
    pub compiler_policy_semantic_revision: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CombatStatModifierIdentityV1 {
    pub id: u32,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CombatStatWholeHandHazardSourceV1 {
    CanonicalLeaderCard {
        key: CardKey,
        name: String,
    },
    Modifier {
        source_kind: CombatStatEffectSourceV1,
        identity: CombatStatModifierIdentityV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CombatStatDisabledReasonV1 {
    OrdinaryAbility {
        registry_reasons: Box<[UnsupportedReasonV1]>,
    },
    OutOfSliceBonus {
        registry_reasons: Box<[UnsupportedReasonV1]>,
    },
    UnsupportedPromisedControl {
        registry_reasons: Box<[UnsupportedReasonV1]>,
    },
    UnsupportedSelectedHazard {
        registry_reasons: Box<[UnsupportedReasonV1]>,
    },
    CappedIncrease {
        registry_reasons: Box<[UnsupportedReasonV1]>,
    },
    /// Support ability shape outside the projection's reviewed unconditional basic-stat
    /// subset. Retained as a distinct visible reason for compatibility and diagnostics.
    SupportAbility {
        registry_reasons: Box<[UnsupportedReasonV1]>,
    },
    UnsupportedPostRoundRecovery {
        registry_reasons: Box<[UnsupportedReasonV1]>,
    },
    UnsupportedPostRoundResourceEffect {
        registry_reasons: Box<[UnsupportedReasonV1]>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CombatStatProjectionDispositionV1 {
    Absent,
    Execute {
        identity: CombatStatModifierIdentityV1,
        effect: SupportedEffectV1,
        predicate: CombatStatPredicateV1,
    },
    ExecutePostRound {
        identity: CombatStatModifierIdentityV1,
        effect: CombatStatPostRoundEffectV1,
        /// The condition the effect's own printed text names, or `Always`. A post-round
        /// plan may be conditional, so a preparation report has to say so: without this a
        /// reviewed conditional reduction would read exactly like an unconditional one.
        predicate: CombatStatPredicateV1,
    },
    Disabled {
        identity: CombatStatModifierIdentityV1,
        reason: CombatStatDisabledReasonV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CombatStatCardPreparationV1 {
    pub key: CardKey,
    pub effective_clan_id: u32,
    pub effective_clan_character_count: u16,
    pub source_bonus_support_count: u16,
    pub source_ability_support_count: u16,
    pub ability: CombatStatProjectionDispositionV1,
    pub bonus: CombatStatProjectionDispositionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CombatStatDiagnosticSelectedCardReportV1 {
    pub key: CardKey,
    pub hand_slot: u8,
    pub effective_clan_id: u32,
    pub effective_clan_character_count: u16,
    pub source_bonus_support_count: u16,
    pub source_ability_support_count: u16,
    pub ability: CombatStatProjectionDispositionV1,
    pub bonus: CombatStatProjectionDispositionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CombatStatDiagnosticRoundReportV1 {
    pub model: CombatStatReplayModelV1,
    pub provenance: CombatStatDiagnosticProvenanceV1,
    pub round: BaseRulesRoundReport,
    pub selected: ByPlayer<CombatStatDiagnosticSelectedCardReportV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CombatStatDiagnosticReplayReportV1 {
    pub battle_id: u64,
    pub model: CombatStatReplayModelV1,
    pub provenance: CombatStatDiagnosticProvenanceV1,
    pub rounds: Vec<CombatStatDiagnosticRoundReportV1>,
    pub final_position: BaseRulesPosition,
}

#[derive(Debug)]
pub enum CombatStatDiagnosticPreparationErrorV1 {
    Replay(ReplayValidationError),
    Lookup {
        battle_id: u64,
        player: PlayerId,
        hand_slot: u8,
        source_kind: CombatStatEffectSourceV1,
        source: EffectLookupError,
    },
    UnsupportedCompiledShape {
        battle_id: u64,
        player: PlayerId,
        hand_slot: u8,
        source_kind: CombatStatEffectSourceV1,
        effect_id: u32,
    },
    WholeHandExecutionHazard {
        battle_id: u64,
        player: PlayerId,
        hand_slot: u8,
        source: CombatStatWholeHandHazardSourceV1,
    },
    EnginePlan(CombatStatPlanErrorV1),
}

impl fmt::Display for CombatStatDiagnosticPreparationErrorV1 {
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
                model = CombatStatReplayModelV1::CombatStatDiagnosticV1
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
                model = CombatStatReplayModelV1::CombatStatDiagnosticV1
            ),
            Self::WholeHandExecutionHazard {
                battle_id,
                player,
                hand_slot,
                source,
            } => write!(
                formatter,
                "{} battle {battle_id} {player:?} slot {hand_slot} whole-hand execution hazard {source:?}",
                CombatStatReplayModelV1::CombatStatDiagnosticV1,
            ),
            Self::EnginePlan(source) => write!(
                formatter,
                "{} preparation failed: {source}",
                CombatStatReplayModelV1::CombatStatDiagnosticV1
            ),
        }
    }
}

impl Error for CombatStatDiagnosticPreparationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Replay(source) => Some(source),
            Self::Lookup { source, .. } => Some(source),
            Self::EnginePlan(source) => Some(source),
            Self::UnsupportedCompiledShape { .. } | Self::WholeHandExecutionHazard { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CombatStatDiagnosticReplayErrorV1 {
    PrefixOutOfRange {
        battle_id: u64,
        requested: usize,
        available: usize,
    },
    Engine {
        context: BaseRulesReplayRoundContext,
        selected: ByPlayer<CombatStatDiagnosticSelectedCardReportV1>,
        source: CombatStatDiagnosticErrorV1,
    },
    Mismatch {
        context: BaseRulesReplayRoundContext,
        selected: ByPlayer<CombatStatDiagnosticSelectedCardReportV1>,
        field: String,
        expected: String,
        actual: String,
    },
}

impl fmt::Display for CombatStatDiagnosticReplayErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let model = CombatStatReplayModelV1::CombatStatDiagnosticV1;
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

impl Error for CombatStatDiagnosticReplayErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Engine { source, .. } => Some(source),
            Self::PrefixOutOfRange { .. } | Self::Mismatch { .. } => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CombatStatDiagnosticReplayV1 {
    base: BaseRulesReplay,
    match_spec: CombatStatDiagnosticMatchSpecV1,
    cards: ByPlayer<[CombatStatCardPreparationV1; HAND_SIZE]>,
    provenance: CombatStatDiagnosticProvenanceV1,
}

struct PreparedCombatStatSourceV1 {
    disposition: CombatStatProjectionDispositionV1,
    compact_plan: CombatStatSourcePlanV1,
}

struct PreparedCombatStatCardV1 {
    metadata: CombatStatCardPreparationV1,
    compact_plan: CombatStatCardPlanV1,
}

struct PreparedCombatStatCardsV1 {
    metadata: ByPlayer<[CombatStatCardPreparationV1; HAND_SIZE]>,
    compact_plans: ByPlayer<[CombatStatCardPlanV1; HAND_SIZE]>,
}

impl CombatStatDiagnosticReplayV1 {
    pub fn new(
        replay: ReplayCaseV1,
        catalog: &CardCatalog,
        registry: &EffectRegistryV1,
        projection: CombatStatDiagnosticProjectionV1,
    ) -> Result<Self, CombatStatDiagnosticPreparationErrorV1> {
        let base = BaseRulesReplay::new(replay, catalog)
            .map_err(CombatStatDiagnosticPreparationErrorV1::Replay)?;
        let battle_id = base.battle_id();
        let prepared = prepare_combat_stat_cards(base.replay(), catalog, registry, battle_id)?;
        let match_spec = CombatStatDiagnosticMatchSpecV1 {
            base_rules: base.match_spec().clone(),
            cards: prepared.compact_plans,
        };
        CombatStatDiagnosticV1::new(match_spec.clone())
            .map_err(CombatStatDiagnosticPreparationErrorV1::EnginePlan)?;
        let provenance = CombatStatDiagnosticProvenanceV1 {
            model: CombatStatReplayModelV1::CombatStatDiagnosticV1,
            projection,
            effect_registry_schema_version: registry.schema_version(),
            effect_registry_source_fingerprint_fnv1a64: registry.source_fingerprint_fnv1a64(),
            compiler_policy_semantic_revision:
                COMBAT_STAT_DIAGNOSTIC_COMPILER_POLICY_SEMANTIC_REVISION_V1,
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

    pub fn preparation(&self) -> &ByPlayer<[CombatStatCardPreparationV1; HAND_SIZE]> {
        &self.cards
    }

    pub const fn preparation_provenance(&self) -> CombatStatDiagnosticProvenanceV1 {
        self.provenance
    }

    pub fn new_game(&self) -> CombatStatDiagnosticV1 {
        CombatStatDiagnosticV1::new(self.match_spec.clone())
            .expect("the immutable combat-stat diagnostic match plan was validated at construction")
    }

    pub fn execute_combat_stat_diagnostic_v1(
        &self,
    ) -> Result<CombatStatDiagnosticReplayReportV1, CombatStatDiagnosticReplayErrorV1> {
        self.execute_combat_stat_diagnostic_v1_prefix(self.replay().rounds.len())
    }

    pub fn execute_combat_stat_diagnostic_v1_prefix(
        &self,
        rounds: usize,
    ) -> Result<CombatStatDiagnosticReplayReportV1, CombatStatDiagnosticReplayErrorV1> {
        if rounds > self.replay().rounds.len() {
            return Err(CombatStatDiagnosticReplayErrorV1::PrefixOutOfRange {
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
                    .map_err(|source| CombatStatDiagnosticReplayErrorV1::Engine {
                        context,
                        selected: selected.clone(),
                        source,
                    })?;
            assert_combat_stat_round(replay_round, &report, context, &selected)?;
            reports.push(CombatStatDiagnosticRoundReportV1 {
                model: CombatStatReplayModelV1::CombatStatDiagnosticV1,
                provenance: self.provenance,
                round: report,
                selected,
            });
        }
        Ok(CombatStatDiagnosticReplayReportV1 {
            battle_id: self.battle_id(),
            model: CombatStatReplayModelV1::CombatStatDiagnosticV1,
            provenance: self.provenance,
            rounds: reports,
            final_position: game.position().clone(),
        })
    }

    fn selected_report(
        &self,
        input: BaseRulesRoundInput,
    ) -> ByPlayer<CombatStatDiagnosticSelectedCardReportV1> {
        let make = |player: PlayerId| {
            let hand_slot = input.selections[player].hand_index;
            let prepared = &self.cards[player][usize::from(hand_slot)];
            CombatStatDiagnosticSelectedCardReportV1 {
                key: prepared.key,
                hand_slot,
                effective_clan_id: prepared.effective_clan_id,
                effective_clan_character_count: prepared.effective_clan_character_count,
                source_bonus_support_count: prepared.source_bonus_support_count,
                source_ability_support_count: prepared.source_ability_support_count,
                ability: prepared.ability.clone(),
                bonus: prepared.bonus.clone(),
            }
        };
        ByPlayer::new(make(PlayerId::P1), make(PlayerId::P2))
    }
}

fn prepare_combat_stat_cards(
    replay: &ReplayCaseV1,
    catalog: &CardCatalog,
    registry: &EffectRegistryV1,
    battle_id: u64,
) -> Result<PreparedCombatStatCardsV1, CombatStatDiagnosticPreparationErrorV1> {
    let mut prepared: ByPlayer<[Option<PreparedCombatStatCardV1>; HAND_SIZE]> =
        ByPlayer::new([const { None }; HAND_SIZE], [const { None }; HAND_SIZE]);
    for player in PlayerId::ALL {
        let effective = derive_effective_catalog_hand(
            std::array::from_fn(|slot| replay.players[player.index()].hand[slot].key),
            catalog,
        )
        .expect("BaseRulesReplay validated every card and catalog clan definition");
        for slot in 0..HAND_SIZE {
            let card = &replay.players[player.index()].hand[slot];
            let canonical = catalog
                .get(card.key)
                .expect("BaseRulesReplay validated every canonical card key");
            if canonical.clan_id == 36 {
                return Err(
                    CombatStatDiagnosticPreparationErrorV1::WholeHandExecutionHazard {
                        battle_id,
                        player,
                        hand_slot: slot as u8,
                        source: CombatStatWholeHandHazardSourceV1::CanonicalLeaderCard {
                            key: card.key,
                            name: canonical.name.clone(),
                        },
                    },
                );
            }
            let source_bonus_support_count = if card.source_bonus.is_some() {
                effective[slot].source_bonus_support_count
            } else {
                0
            };
            let ability = prepare_combat_stat_source(
                registry,
                battle_id,
                player,
                slot as u8,
                CombatStatEffectSourceV1::Ability,
                card.source_ability.as_ref(),
            )?;
            let bonus = prepare_combat_stat_source(
                registry,
                battle_id,
                player,
                slot as u8,
                CombatStatEffectSourceV1::Bonus,
                card.source_bonus.as_ref(),
            )?;
            prepared[player][slot] = Some(PreparedCombatStatCardV1 {
                metadata: CombatStatCardPreparationV1 {
                    key: card.key,
                    effective_clan_id: effective[slot].effective_clan_id,
                    effective_clan_character_count: effective[slot].effective_clan_character_count,
                    source_bonus_support_count,
                    source_ability_support_count: executable_ability_support_count(
                        ability.compact_plan,
                        effective[slot].effective_clan_character_count,
                    ),
                    ability: ability.disposition,
                    bonus: bonus.disposition,
                },
                compact_plan: CombatStatCardPlanV1 {
                    key: card.key,
                    effective_clan_id: effective[slot].effective_clan_id,
                    source_bonus_support_count,
                    source_ability_support_count: executable_ability_support_count(
                        ability.compact_plan,
                        effective[slot].effective_clan_character_count,
                    ),
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
    Ok(PreparedCombatStatCardsV1 {
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

fn prepare_combat_stat_source(
    registry: &EffectRegistryV1,
    battle_id: u64,
    player: PlayerId,
    hand_slot: u8,
    source_kind: CombatStatEffectSourceV1,
    source: Option<&super::model::SourceModifier>,
) -> Result<PreparedCombatStatSourceV1, CombatStatDiagnosticPreparationErrorV1> {
    let Some(source) = source else {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::Absent,
            compact_plan: CombatStatSourcePlanV1::Absent,
        });
    };
    let definition = registry
        .lookup_capture(source.id, &source.description)
        .map_err(|error| CombatStatDiagnosticPreparationErrorV1::Lookup {
            battle_id,
            player,
            hand_slot,
            source_kind,
            source: error,
        })?;
    let identity = CombatStatModifierIdentityV1 {
        id: source.id,
        description: source.description.clone(),
    };
    if is_whole_hand_modifier_hazard(definition) {
        return Err(
            CombatStatDiagnosticPreparationErrorV1::WholeHandExecutionHazard {
                battle_id,
                player,
                hand_slot,
                source: CombatStatWholeHandHazardSourceV1::Modifier {
                    source_kind,
                    identity,
                },
            },
        );
    }
    if classify_defeat_recover_pillz(definition, source_kind) {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::RecoverPaidPillzOnDefeat,
                predicate: CombatStatPredicateV1::Always,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate: CombatStatPredicateV1::Always,
                effect: CombatStatEffectV1::RecoverPaidPillzOnDefeat,
            },
        });
    }
    if classify_argos_defeat_capped_pillz(definition, source_kind) {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::GainTwoPillzOnDefeatMaxEleven,
                predicate: CombatStatPredicateV1::Always,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate: CombatStatPredicateV1::Always,
                effect: CombatStatEffectV1::GainTwoPillzOnDefeatMaxEleven,
            },
        });
    }
    // Anita is deliberately an identity-locked post-round conversion rather than a
    // generic Life increase: Courage is bound into the compact predicate and the engine
    // supplies the selected card's final resolved damage as the runtime magnitude.
    if classify_anita_courage_damage_to_life(definition, source_kind) {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
                predicate: CombatStatPredicateV1::OwnerMovesFirst,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate: CombatStatPredicateV1::OwnerMovesFirst,
                effect: CombatStatEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
            },
        });
    }
    // The reviewed Victory opponent-Life reductions are a typed post-round plan, not a
    // combat-stat modifier, so normal Stop liveness still applies but cancellation never
    // reinterprets their magnitude. The conditional members carry the predicate their own
    // printed text names, which the engine resolves before the round is prepared.
    if let Some((life, minimum, predicate)) =
        classify_victory_opponent_life(definition, source_kind)
    {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictory { life, minimum },
                predicate,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate,
                effect: CombatStatEffectV1::ReduceOpponentLifeOnVictory { life, minimum },
            },
        });
    }
    if classify_victory_or_defeat_pillz(definition, source_kind) {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::GainOnePillzOnVictoryOrDefeat,
                predicate: CombatStatPredicateV1::Always,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate: CombatStatPredicateV1::Always,
                effect: CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat,
            },
        });
    }
    if let Some(effect) = classify_victory_or_defeat_life(definition, source_kind) {
        let (post_round_effect, compact_effect) = match effect {
            VictoryOrDefeatLifeEffectV1::GainLife { life } => (
                CombatStatPostRoundEffectV1::GainLifeOnVictoryOrDefeat { life },
                CombatStatEffectV1::GainLifeOnVictoryOrDefeat { life },
            ),
            VictoryOrDefeatLifeEffectV1::ReduceOpponentLife { life, minimum } => (
                CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryOrDefeat { life, minimum },
                CombatStatEffectV1::ReduceOpponentLifeOnVictoryOrDefeat { life, minimum },
            ),
        };
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: post_round_effect,
                predicate: CombatStatPredicateV1::Always,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate: CombatStatPredicateV1::Always,
                effect: compact_effect,
            },
        });
    }
    if let Some((per_star, minimum)) =
        classify_equalizer_opponent_life_on_victory(definition, source_kind)
    {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars {
                    per_star,
                    minimum,
                },
                predicate: CombatStatPredicateV1::Always,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate: CombatStatPredicateV1::Always,
                effect: CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars {
                    per_star,
                    minimum,
                },
            },
        });
    }
    if classify_komboka_victory_pillz_and_life(definition, source_kind) {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::GainOnePillzAndLifeOnVictory,
                predicate: CombatStatPredicateV1::Always,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate: CombatStatPredicateV1::Always,
                effect: CombatStatEffectV1::GainOnePillzAndLifeOnVictory,
            },
        });
    }
    if let Some(life) = classify_victory_life(definition, source_kind) {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::GainLifeOnVictory { life },
                predicate: CombatStatPredicateV1::Always,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate: CombatStatPredicateV1::Always,
                effect: CombatStatEffectV1::GainLifeOnVictory { life },
            },
        });
    }
    if let Some(life) = classify_defeat_life(definition, source_kind) {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::GainLifeOnDefeat { life },
                predicate: CombatStatPredicateV1::Always,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate: CombatStatPredicateV1::Always,
                effect: CombatStatEffectV1::GainLifeOnDefeat { life },
            },
        });
    }
    // The generic structural classifier makes malformed Reanimate records fail-closed, but
    // the executable replay slice remains restricted to Lobo's captured identity.
    if source.id == LOBO_REANIMATE_REGISTRY_ID {
        if let Some(life) = classify_reanimate_life(definition, source_kind) {
            return Ok(PreparedCombatStatSourceV1 {
                disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                    identity,
                    effect: CombatStatPostRoundEffectV1::ReanimateLife { life },
                    predicate: CombatStatPredicateV1::Always,
                },
                compact_plan: CombatStatSourcePlanV1::Execute {
                    source_id: source.id,
                    predicate: CombatStatPredicateV1::Always,
                    effect: CombatStatEffectV1::ReanimateLife { life },
                },
            });
        }
    }
    if let Some((effect, predicate)) = classify_combat_stat_effect(definition, source_kind) {
        let compact_effect = compact_effect(effect).ok_or(
            CombatStatDiagnosticPreparationErrorV1::UnsupportedCompiledShape {
                battle_id,
                player,
                hand_slot,
                source_kind,
                effect_id: source.id,
            },
        )?;
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::Execute {
                identity,
                effect,
                predicate,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate,
                effect: compact_effect,
            },
        });
    }

    let registry_reasons = definition
        .compiled()
        .unsupported_reasons()
        .to_vec()
        .into_boxed_slice();
    let attempted_control = attempts_promised_control(definition.structured_input());
    let selected_hazard = attempts_selected_hazard(definition);
    let unadmitted_combat_stat = attempts_combat_stat_change(definition.structured_input());
    let unadmitted_post_round_recovery =
        definition.structured_input().special_action == SpecialActionV1::RecoverPillz;
    let unadmitted_victory_or_defeat = source.description == "Victory Or Defeat : +1 Pillz";
    // Victory Or Defeat Life is a closed identity-and-shape family.  Its aliases, malformed
    // records, capped variants, and compound neighbours must all fail when selected rather
    // than silently becoming a disabled no-op.
    let unadmitted_victory_or_defeat_life =
        matches!(source.id, 1396 | 2944 | 2992 | 5799 | 5802 | 5835 | 1628)
            || (source.description.contains("Victory Or Defeat")
                && (source.description.contains("Life")
                    || definition.structured_input().attribute_affected
                        == AttributeAffectedV1::Life));
    // Equalizer opponent-Life is an equally closed identity-and-shape family. Captured
    // Copy can put either reviewed id in either source slot, but every malformed record
    // and adjacent Equalizer Life form must reject when selected.
    let unadmitted_equalizer_opponent_life = matches!(source.id, 1415 | 4458)
        || (source.description.contains("Equalizer")
            && (source.description.contains("Life")
                || definition.structured_input().attribute_affected == AttributeAffectedV1::Life));
    // A near-miss of the admitted family is a selected hazard: either the literal grammar
    // names Victory Life but its structure is wrong, or the complete reviewed structure
    // is present under malformed text. Other Life families keep the diagnostic's existing
    // visible-but-disabled behavior until their own slices are implemented.
    let unadmitted_victory_life = (source.description.starts_with('+')
        && source.description.ends_with(" Life")
        && definition.structured_input().attribute_affected == AttributeAffectedV1::Life)
        || has_victory_life_shape(definition);
    // The admitted grammar is deliberately narrower than the deferred family. Capped,
    // compound, and context-prefixed losing-Life forms must still reject when selected;
    // otherwise a near-miss would quietly become a no-op merely because it is not plain
    // `Defeat: +N Life` / `Reanimate: +N Life` text.
    let input = definition.structured_input();
    let losing_own_life_increase = input.current_round_requirement
        == crate::effect_registry::CurrentRoundRequirementV1::Lose
        && input.side_affected == crate::effect_registry::AffectedSideV1::Player
        && input.attribute_action == AttributeActionV1::Increase
        && matches!(
            input.attribute_affected,
            AttributeAffectedV1::Life | AttributeAffectedV1::LifeAndPillz
        );
    let unadmitted_defeat_life = (source.description.contains("Defeat")
        && losing_own_life_increase)
        || has_defeat_life_shape(definition);
    let unadmitted_reanimate_life = (source.description.contains("Reanimate")
        && losing_own_life_increase)
        || has_reanimate_life_shape(definition);
    // A shape, identity, or description mutation of the reviewed Argos record must remain
    // a selected hazard instead of quietly becoming a disabled no-op.
    let unadmitted_argos_defeat_capped_pillz =
        source.id == 1158 || source.description == "Defeat: +2 Pillz Max. 11";
    // Anita's conversion has one printed Ability identity and one exact structured
    // record. Source-slot changes, card borrowing, same-text aliases, and malformed
    // records are all selected hazards rather than inert disabled sources.
    let unadmitted_anita_courage_damage_to_life = source.id == 274
        || source.description == "Courage: +1 Life Per Dmg"
        || input.special_action == SpecialActionV1::ConvertDamageToLife;
    // A wrong source slot, a malformed record, and the same-text catalog ids that carry
    // no registry definition (Rakhan, Milovan, Fraser) stay selected hazards rather than
    // inert no-ops. Conditional siblings such as Symmetry 4708, Courage 4533, Confidence
    // 3016, and Growth 1730 belong to their own deferred families and keep their existing
    // Disabled records, so this deliberately does not claim the whole structural shape.
    let unadmitted_victory_opponent_life = source.id == 680
        || source.id == 1399
        || source.description == "-5 Opp. Life Min 5"
        || source.description == "-2 Opp. Life Min 2";
    let unadmitted_komboka_victory_pillz_and_life = source.id == 1714
        || source.description == "+1 Pillz And Life"
        || (input.side_affected == crate::effect_registry::AffectedSideV1::Player
            && input.attribute_affected == AttributeAffectedV1::LifeAndPillz
            && input.attribute_action == AttributeActionV1::Increase);
    let reason = if attempted_control {
        CombatStatDisabledReasonV1::UnsupportedPromisedControl { registry_reasons }
    } else if selected_hazard {
        CombatStatDisabledReasonV1::UnsupportedSelectedHazard { registry_reasons }
    } else if unadmitted_victory_or_defeat
        || unadmitted_victory_or_defeat_life
        || unadmitted_equalizer_opponent_life
        || unadmitted_victory_life
        || unadmitted_defeat_life
        || unadmitted_reanimate_life
        || unadmitted_argos_defeat_capped_pillz
        || unadmitted_anita_courage_damage_to_life
        || unadmitted_victory_opponent_life
        || unadmitted_komboka_victory_pillz_and_life
    {
        CombatStatDisabledReasonV1::UnsupportedPostRoundResourceEffect { registry_reasons }
    } else if unadmitted_post_round_recovery {
        CombatStatDisabledReasonV1::UnsupportedPostRoundRecovery { registry_reasons }
    } else if is_capped_increase(definition.structured_input()) {
        CombatStatDisabledReasonV1::CappedIncrease { registry_reasons }
    } else if source_kind == CombatStatEffectSourceV1::Ability
        && definition.structured_input().is_support
    {
        CombatStatDisabledReasonV1::SupportAbility { registry_reasons }
    } else if source_kind == CombatStatEffectSourceV1::Ability {
        CombatStatDisabledReasonV1::OrdinaryAbility { registry_reasons }
    } else {
        CombatStatDisabledReasonV1::OutOfSliceBonus { registry_reasons }
    };
    let compact_plan = if attempted_control
        || selected_hazard
        || unadmitted_combat_stat
        || unadmitted_post_round_recovery
        || unadmitted_victory_or_defeat
        || unadmitted_victory_or_defeat_life
        || unadmitted_equalizer_opponent_life
        || unadmitted_victory_life
        || unadmitted_defeat_life
        || unadmitted_reanimate_life
        || unadmitted_argos_defeat_capped_pillz
        || unadmitted_anita_courage_damage_to_life
        || unadmitted_victory_opponent_life
        || unadmitted_komboka_victory_pillz_and_life
    {
        CombatStatSourcePlanV1::RejectIfSelected {
            source_id: source.id,
        }
    } else {
        CombatStatSourcePlanV1::Disabled {
            source_id: source.id,
        }
    };
    Ok(PreparedCombatStatSourceV1 {
        disposition: CombatStatProjectionDispositionV1::Disabled { identity, reason },
        compact_plan,
    })
}

fn executable_ability_support_count(
    plan: CombatStatSourcePlanV1,
    effective_clan_character_count: u16,
) -> u16 {
    matches!(
        plan,
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::ModifyCombatStat {
                multiplier: CombatStatMagnitudeV1::SourceBonusSupport,
                ..
            },
            ..
        }
    )
    .then_some(effective_clan_character_count)
    .unwrap_or(0)
}

fn is_capped_increase(input: &StructuredEffectV1) -> bool {
    input.attribute_action == AttributeActionV1::Increase && input.value_max != 0
}

fn is_whole_hand_modifier_hazard(definition: &EffectDefinitionV1) -> bool {
    let description = definition.description();
    description.contains("Team:")
        || description.contains("Leader:")
        || description.contains("Global:")
        || definition.structured_input().special_action == SpecialActionV1::Mock
}

fn attempts_selected_hazard(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    matches!(
        input.special_action,
        SpecialActionV1::StopAbility
            | SpecialActionV1::ProtectAbility
            | SpecialActionV1::ProtectBonus
            | SpecialActionV1::CopyAbility
            | SpecialActionV1::CopyBonus
            | SpecialActionV1::RandomAbilities
    ) || matches!(
        input.attribute_action,
        AttributeActionV1::Copy | AttributeActionV1::Protect | AttributeActionV1::Simplify
    ) || definition.description().contains("Exchange")
        || definition.description().contains("Tune Out")
}

fn attempts_combat_stat_change(input: &StructuredEffectV1) -> bool {
    matches!(
        input.attribute_action,
        AttributeActionV1::Increase | AttributeActionV1::Decrease
    ) && matches!(
        input.attribute_affected,
        AttributeAffectedV1::Attack
            | AttributeAffectedV1::Damage
            | AttributeAffectedV1::Power
            | AttributeAffectedV1::PowerAndAttack
            | AttributeAffectedV1::PowerAndDamage
    )
}

fn attempts_promised_control(input: &crate::effect_registry::StructuredEffectV1) -> bool {
    input.special_action == SpecialActionV1::StopBonus
        || input.attribute_action == AttributeActionV1::StopModifier
}

fn assert_combat_stat_round(
    expected: &ReplayRound,
    actual: &BaseRulesRoundReport,
    context: BaseRulesReplayRoundContext,
    selected: &ByPlayer<CombatStatDiagnosticSelectedCardReportV1>,
) -> Result<(), CombatStatDiagnosticReplayErrorV1> {
    for player in PlayerId::ALL {
        let expected_player = expected.expected_player_states[player.index()];
        compare_combat_stat_field(
            context,
            selected,
            format!("players.{player:?}.life"),
            expected_player.life,
            actual.players[player].life,
        )?;
        compare_combat_stat_field(
            context,
            selected,
            format!("players.{player:?}.pillz"),
            expected_player.pillz,
            actual.players[player].pillz,
        )?;
        if let Some(expected_card) = expected.expected_card_results[player.index()] {
            let actual_card = actual.cards[player];
            compare_combat_stat_field(
                context,
                selected,
                format!("cards.{player:?}.power"),
                expected_card.power,
                actual_card.power,
            )?;
            compare_combat_stat_field(
                context,
                selected,
                format!("cards.{player:?}.damage"),
                expected_card.damage,
                actual_card.damage,
            )?;
            compare_combat_stat_field(
                context,
                selected,
                format!("cards.{player:?}.attack"),
                expected_card.attack,
                actual_card.attack,
            )?;
            compare_combat_stat_field(
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

fn compare_combat_stat_field<T: fmt::Debug + PartialEq>(
    context: BaseRulesReplayRoundContext,
    selected: &ByPlayer<CombatStatDiagnosticSelectedCardReportV1>,
    field: String,
    expected: T,
    actual: T,
) -> Result<(), CombatStatDiagnosticReplayErrorV1> {
    if expected == actual {
        Ok(())
    } else {
        Err(CombatStatDiagnosticReplayErrorV1::Mismatch {
            context,
            selected: selected.clone(),
            field,
            expected: format!("{expected:?}"),
            actual: format!("{actual:?}"),
        })
    }
}
