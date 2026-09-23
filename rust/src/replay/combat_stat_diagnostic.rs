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
    classify_bet_gated_post_round, classify_both_players_life_reduction, classify_brawl_post_round,
    classify_combat_stat_effect, classify_defeat_life, classify_defeat_opponent_life,
    classify_defeat_opponent_pillz, classify_defeat_pillz, classify_defeat_pillz_and_life,
    classify_defeat_recover_pillz, classify_equalizer_opponent_life_on_victory,
    classify_equalizer_post_round_gain, classify_heal_life_on_victory,
    classify_killshot_opponent_life, classify_killshot_pillz_and_life,
    classify_komboka_victory_pillz_and_life, classify_poison_opponent_life_on_defeat,
    classify_poison_opponent_life_on_victory, classify_reanimate_life,
    classify_regen_life_on_victory, classify_round_scaled_post_round, classify_support_post_round,
    classify_toxin_opponent_life_on_victory, classify_victory_life,
    classify_victory_life_per_damage, classify_victory_life_per_opponent_damage,
    classify_victory_opponent_life, classify_victory_opponent_pillz,
    classify_victory_or_defeat_both_players_gain, classify_victory_or_defeat_life,
    classify_victory_or_defeat_pillz, classify_victory_pillz, classify_victory_pillz_per_damage,
    compact_effect, has_bet_gated_post_round_shape, has_both_players_life_reduction_shape,
    has_brawl_post_round_shape, has_defeat_life_shape, has_defeat_opponent_pillz_shape,
    has_defeat_pillz_shape, has_equalizer_post_round_shape, has_heal_life_on_victory_shape,
    has_killshot_opponent_life_shape, has_poison_opponent_life_on_defeat_shape,
    has_poison_opponent_life_on_victory_shape, has_reanimate_life_shape,
    has_regen_life_on_victory_shape, has_round_scaled_post_round_shape,
    has_support_post_round_shape, has_toxin_opponent_life_on_victory_shape, has_victory_life_shape,
    has_victory_opponent_life_shape, has_victory_opponent_pillz_shape,
    has_victory_or_defeat_both_players_gain_shape, has_victory_or_defeat_opponent_life_shape,
    has_victory_pillz_per_damage_shape, has_victory_pillz_shape, BothPlayersGainV1,
    VictoryOrDefeatLifeEffectV1, COMBAT_STAT_COMPILER_POLICY_SEMANTIC_REVISION_V1,
};
use crate::engine::{
    derive_effective_catalog_hand, effect_reads_support_count, unmodelled_source_context,
    BaseRulesPosition, BaseRulesRoundInput, BaseRulesRoundReport, ByPlayer, CombatStatCardPlanV1,
    CombatStatDiagnosticErrorV1, CombatStatDiagnosticMatchSpecV1, CombatStatDiagnosticV1,
    CombatStatEffectSourceV1, CombatStatEffectV1, CombatStatPlanErrorV1,
    CombatStatPostRoundEffectV1, CombatStatPredicateV1, CombatStatSourcePlanV1, PlayerId,
    HAND_SIZE,
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

/// Some sources are admitted only where their context is one the corpus has pinned - a
/// `Stop:` source nothing opposite can stop, a resource canceller facing nothing whose
/// cancellation is unpinned, a `/ Life Lost` magnitude whose owner's Life cannot rise,
/// `Tune Out` where no Killshot or opposing Power/Attack cancel can meet it - and the
/// engine refuses a plan that puts them elsewhere. A
/// capture is concrete, though, so rather than refusing the whole replay such a source
/// becomes a selected hazard: a round that selects it is refused, and every other round is
/// still checked against the server.
fn downgrade_unmodelled_stop_triggered_sources(prepared: &mut PreparedCombatStatCardsV1) {
    for player in PlayerId::ALL {
        let opponent = prepared.compact_plans[player.other()];
        let own = prepared.compact_plans[player];
        for slot in 0..HAND_SIZE {
            for bonus in [false, true] {
                let card = &mut prepared.compact_plans[player][slot];
                let plan = if bonus { card.bonus } else { card.ability };
                if unmodelled_source_context(plan, &own, &opponent).is_none() {
                    continue;
                }
                let CombatStatSourcePlanV1::Execute { source_id, .. } = plan else {
                    continue;
                };
                let hazard = CombatStatSourcePlanV1::RejectIfSelected { source_id };
                let metadata = &mut prepared.metadata[player][slot];
                let disposition = if bonus {
                    card.bonus = hazard;
                    &mut metadata.bonus
                } else {
                    card.ability = hazard;
                    &mut metadata.ability
                };
                if let CombatStatProjectionDispositionV1::Execute { identity, .. } = disposition {
                    *disposition = CombatStatProjectionDispositionV1::Disabled {
                        identity: identity.clone(),
                        reason: CombatStatDisabledReasonV1::UnsupportedSelectedHazard {
                            registry_reasons: Box::new([]),
                        },
                    };
                }
            }
        }
    }
}

fn downgrade_ambiguous_clan_gates(
    prepared: &mut PreparedCombatStatCardsV1,
    base_rules: &crate::engine::BaseRulesMatchSpec,
) {
    let plans = prepared.compact_plans;
    for player in PlayerId::ALL {
        for slot in 0..HAND_SIZE {
            for bonus in [false, true] {
                let plan = if bonus {
                    plans[player][slot].bonus
                } else {
                    plans[player][slot].ability
                };
                let CombatStatSourcePlanV1::Execute {
                    source_id,
                    predicate:
                        CombatStatPredicateV1::OwnerPreviousCardClanIn(set)
                        | CombatStatPredicateV1::OpponentHandHasClan(set),
                    ..
                } = plan
                else {
                    continue;
                };
                if !crate::engine::clan_gate_is_ambiguous(set, base_rules, &plans) {
                    continue;
                }
                let reject = CombatStatSourcePlanV1::RejectIfSelected { source_id };
                let metadata = if bonus {
                    prepared.compact_plans[player][slot].bonus = reject;
                    &mut prepared.metadata[player][slot].bonus
                } else {
                    prepared.compact_plans[player][slot].ability = reject;
                    &mut prepared.metadata[player][slot].ability
                };
                if let CombatStatProjectionDispositionV1::Execute { identity, .. } = metadata {
                    *metadata = CombatStatProjectionDispositionV1::Disabled {
                        identity: identity.clone(),
                        reason: CombatStatDisabledReasonV1::UnsupportedSelectedHazard {
                            registry_reasons: Box::new([]),
                        },
                    };
                }
            }
        }
    }
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
        let mut prepared = prepare_combat_stat_cards(base.replay(), catalog, registry, battle_id)?;
        downgrade_unmodelled_stop_triggered_sources(&mut prepared);
        downgrade_ambiguous_clan_gates(&mut prepared, base.match_spec());
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

/// Pair a post-round grammar's provenance disposition with the compact plan the hot path
/// executes. Every admitted grammar records the same two-sided result from the same parts,
/// and building both here is what keeps them from disagreeing: a grammar hands over its
/// public effect, its compact effect and its predicate together, or not at all.
fn executes_post_round(
    identity: CombatStatModifierIdentityV1,
    source_id: u32,
    effect: CombatStatPostRoundEffectV1,
    compact_effect: CombatStatEffectV1,
    predicate: CombatStatPredicateV1,
) -> PreparedCombatStatSourceV1 {
    PreparedCombatStatSourceV1 {
        disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
            identity,
            effect,
            predicate,
        },
        compact_plan: CombatStatSourcePlanV1::Execute {
            source_id,
            predicate,
            effect: compact_effect,
        },
    }
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
        return Ok(executes_post_round(
            identity,
            source.id,
            CombatStatPostRoundEffectV1::RecoverPaidPillzOnDefeat,
            CombatStatEffectV1::RecoverPaidPillzOnDefeat,
            CombatStatPredicateV1::Always,
        ));
    }
    if classify_argos_defeat_capped_pillz(definition, source_kind) {
        return Ok(executes_post_round(
            identity,
            source.id,
            CombatStatPostRoundEffectV1::GainTwoPillzOnDefeatMaxEleven,
            CombatStatEffectV1::GainTwoPillzOnDefeatMaxEleven,
            CombatStatPredicateV1::Always,
        ));
    }
    // Anita is deliberately an identity-locked post-round conversion rather than a
    // generic Life increase: Courage is bound into the compact predicate and the engine
    // supplies the selected card's final resolved damage as the runtime magnitude.
    if classify_anita_courage_damage_to_life(definition, source_kind) {
        return Ok(executes_post_round(
            identity,
            source.id,
            CombatStatPostRoundEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
            CombatStatEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
            CombatStatPredicateV1::OwnerMovesFirst,
        ));
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
    // Its losing-side sibling shares the channel and carries no condition of its own
    // beyond the outcome the engine already resolves.
    if let Some((life, minimum)) = classify_defeat_opponent_life(definition, source_kind) {
        return Ok(executes_post_round(
            identity,
            source.id,
            CombatStatPostRoundEffectV1::ReduceOpponentLifeOnDefeat { life, minimum },
            CombatStatEffectV1::ReduceOpponentLifeOnDefeat { life, minimum },
            CombatStatPredicateV1::Always,
        ));
    }
    // The Killshot sibling likewise carries no condition of its own: the attack ratio is
    // resolved by the engine, not by the plan's predicate.
    if let Some((life, minimum)) = classify_killshot_opponent_life(definition, source_kind) {
        return Ok(executes_post_round(
            identity,
            source.id,
            CombatStatPostRoundEffectV1::ReduceOpponentLifeOnKillshot { life, minimum },
            CombatStatEffectV1::ReduceOpponentLifeOnKillshot { life, minimum },
            CombatStatPredicateV1::Always,
        ));
    }
    if let Some((effect, predicate)) = classify_bet_gated_post_round(definition, source_kind) {
        let (post_round_effect, compact_effect) = effect.effects();
        return Ok(executes_post_round(
            identity,
            source.id,
            post_round_effect,
            compact_effect,
            predicate,
        ));
    }
    if let Some((scale, effect)) = classify_round_scaled_post_round(definition, source_kind) {
        let (post_round_effect, compact_effect) = effect.effects(scale);
        return Ok(executes_post_round(
            identity,
            source.id,
            post_round_effect,
            compact_effect,
            CombatStatPredicateV1::Always,
        ));
    }
    if let Some(amount) = classify_killshot_pillz_and_life(definition, source_kind) {
        return Ok(executes_post_round(
            identity,
            source.id,
            CombatStatPostRoundEffectV1::GainPillzAndLifeOnKillshot { amount },
            CombatStatEffectV1::GainPillzAndLifeOnKillshot { amount },
            CombatStatPredicateV1::Always,
        ));
    }
    if let Some((resource, amount)) =
        classify_victory_or_defeat_both_players_gain(definition, source_kind)
    {
        let (post_round, effect) = match resource {
            BothPlayersGainV1::Life => (
                CombatStatPostRoundEffectV1::GainBothPlayersLifeOnVictoryOrDefeat { life: amount },
                CombatStatEffectV1::GainBothPlayersLifeOnVictoryOrDefeat { life: amount },
            ),
            BothPlayersGainV1::Pillz => (
                CombatStatPostRoundEffectV1::GainBothPlayersPillzOnVictoryOrDefeat {
                    pillz: amount,
                },
                CombatStatEffectV1::GainBothPlayersPillzOnVictoryOrDefeat { pillz: amount },
            ),
        };
        return Ok(executes_post_round(
            identity,
            source.id,
            post_round,
            effect,
            CombatStatPredicateV1::Always,
        ));
    }
    // Xantiax has no outcome channel and no beneficiary: both players pay it whatever the
    // round did, so the plan carries no predicate and the engine reads no winner.
    if let Some((life, minimum)) = classify_both_players_life_reduction(definition, source_kind) {
        return Ok(executes_post_round(
            identity,
            source.id,
            CombatStatPostRoundEffectV1::ReduceBothPlayersLife { life, minimum },
            CombatStatEffectV1::ReduceBothPlayersLife { life, minimum },
            CombatStatPredicateV1::Always,
        ));
    }
    if classify_victory_or_defeat_pillz(definition, source_kind) {
        return Ok(executes_post_round(
            identity,
            source.id,
            CombatStatPostRoundEffectV1::GainOnePillzOnVictoryOrDefeat,
            CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat,
            CombatStatPredicateV1::Always,
        ));
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
        return Ok(executes_post_round(
            identity,
            source.id,
            post_round_effect,
            compact_effect,
            CombatStatPredicateV1::Always,
        ));
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
        return Ok(executes_post_round(
            identity,
            source.id,
            CombatStatPostRoundEffectV1::GainOnePillzAndLifeOnVictory,
            CombatStatEffectV1::GainOnePillzAndLifeOnVictory,
            CombatStatPredicateV1::Always,
        ));
    }
    if let Some((life, predicate)) = classify_victory_life(definition, source_kind) {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::GainLifeOnVictory { life },
                predicate,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate,
                effect: CombatStatEffectV1::GainLifeOnVictory { life },
            },
        });
    }
    if let Some((pillz, predicate)) = classify_victory_pillz(definition, source_kind) {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::GainPillzOnVictory { pillz },
                predicate,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate,
                effect: CombatStatEffectV1::GainPillzOnVictory { pillz },
            },
        });
    }
    if let Some((pillz, minimum)) = classify_victory_opponent_pillz(definition, source_kind) {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::ReduceOpponentPillzOnVictory {
                    pillz,
                    minimum,
                },
                predicate: CombatStatPredicateV1::Always,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate: CombatStatPredicateV1::Always,
                effect: CombatStatEffectV1::ReduceOpponentPillzOnVictory { pillz, minimum },
            },
        });
    }
    if let Some(brawl) = classify_brawl_post_round(definition, source_kind) {
        let (post_round_effect, compact_effect) = brawl.effects();
        return Ok(executes_post_round(
            identity,
            source.id,
            post_round_effect,
            compact_effect,
            CombatStatPredicateV1::Always,
        ));
    }
    // The post-round Support grammars read the owner's Support count, which this card's
    // plan carries as its ability Support context; the Equalizer own gains read the stars.
    if let Some(support) = classify_support_post_round(definition, source_kind) {
        let (post_round_effect, compact_effect) = support.effects();
        return Ok(executes_post_round(
            identity,
            source.id,
            post_round_effect,
            compact_effect,
            CombatStatPredicateV1::Always,
        ));
    }
    if let Some(gain) = classify_equalizer_post_round_gain(definition, source_kind) {
        let (post_round_effect, compact_effect) = gain.effects();
        return Ok(executes_post_round(
            identity,
            source.id,
            post_round_effect,
            compact_effect,
            CombatStatPredicateV1::Always,
        ));
    }
    if let Some((pillz, minimum)) = classify_defeat_opponent_pillz(definition, source_kind) {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::ReduceOpponentPillzOnDefeat { pillz, minimum },
                predicate: CombatStatPredicateV1::Always,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate: CombatStatPredicateV1::Always,
                effect: CombatStatEffectV1::ReduceOpponentPillzOnDefeat { pillz, minimum },
            },
        });
    }
    if let Some(predicate) = classify_victory_pillz_per_damage(definition, source_kind) {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::GainPillzEqualToFinalDamageOnVictory,
                predicate,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate,
                effect: CombatStatEffectV1::GainPillzEqualToFinalDamageOnVictory,
            },
        });
    }
    if let Some(life_per_damage) =
        classify_victory_life_per_opponent_damage(definition, source_kind)
    {
        return Ok(executes_post_round(
            identity,
            source.id,
            CombatStatPostRoundEffectV1::GainLifePerOpponentFinalDamageOnVictory {
                life_per_damage,
            },
            CombatStatEffectV1::GainLifePerOpponentFinalDamageOnVictory { life_per_damage },
            CombatStatPredicateV1::Always,
        ));
    }
    if let Some((life_per_damage, maximum, predicate)) =
        classify_victory_life_per_damage(definition, source_kind)
    {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::GainLifePerFinalDamageOnVictory {
                    life_per_damage,
                    maximum,
                },
                predicate,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate,
                effect: CombatStatEffectV1::GainLifePerFinalDamageOnVictory {
                    life_per_damage,
                    maximum,
                },
            },
        });
    }
    if let Some(life) = classify_defeat_life(definition, source_kind) {
        return Ok(executes_post_round(
            identity,
            source.id,
            CombatStatPostRoundEffectV1::GainLifeOnDefeat { life },
            CombatStatEffectV1::GainLifeOnDefeat { life },
            CombatStatPredicateV1::Always,
        ));
    }
    if let Some(pillz) = classify_defeat_pillz(definition, source_kind) {
        return Ok(executes_post_round(
            identity,
            source.id,
            CombatStatPostRoundEffectV1::GainPillzOnDefeat { pillz },
            CombatStatEffectV1::GainPillzOnDefeat { pillz },
            CombatStatPredicateV1::Always,
        ));
    }
    if let Some(amount) = classify_defeat_pillz_and_life(definition, source_kind) {
        return Ok(executes_post_round(
            identity,
            source.id,
            CombatStatPostRoundEffectV1::GainPillzAndLifeOnDefeat { amount },
            CombatStatEffectV1::GainPillzAndLifeOnDefeat { amount },
            CombatStatPredicateV1::Always,
        ));
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
    // The four admitted permanent grammars. Each plan is ordinary post-round work at the
    // source; the engine position carries the latch from the round it wins onward.
    if let Some((life, maximum, predicate)) = classify_heal_life_on_victory(definition, source_kind)
    {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::HealLifeOnVictory { life, maximum },
                predicate,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate,
                effect: CombatStatEffectV1::HealLifeOnVictory { life, maximum },
            },
        });
    }
    if let Some((life, maximum, predicate)) =
        classify_regen_life_on_victory(definition, source_kind)
    {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::RegenLifeOnVictory { life, maximum },
                predicate,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate,
                effect: CombatStatEffectV1::RegenLifeOnVictory { life, maximum },
            },
        });
    }
    if let Some((life, minimum, predicate)) =
        classify_poison_opponent_life_on_victory(definition, source_kind)
    {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::PoisonOpponentLifeOnVictory { life, minimum },
                predicate,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate,
                effect: CombatStatEffectV1::PoisonOpponentLifeOnVictory { life, minimum },
            },
        });
    }
    // The losing-side latch. Its outcome is the condition, so the plan carries no predicate
    // and the engine reads the round's loser.
    if let Some((life, minimum)) = classify_poison_opponent_life_on_defeat(definition, source_kind)
    {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::PoisonOpponentLifeOnDefeat { life, minimum },
                predicate: CombatStatPredicateV1::Always,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate: CombatStatPredicateV1::Always,
                effect: CombatStatEffectV1::PoisonOpponentLifeOnDefeat { life, minimum },
            },
        });
    }
    if let Some((life, minimum, predicate)) =
        classify_toxin_opponent_life_on_victory(definition, source_kind)
    {
        return Ok(PreparedCombatStatSourceV1 {
            disposition: CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: CombatStatPostRoundEffectV1::ToxinOpponentLifeOnVictory { life, minimum },
                predicate,
            },
            compact_plan: CombatStatSourcePlanV1::Execute {
                source_id: source.id,
                predicate,
                effect: CombatStatEffectV1::ToxinOpponentLifeOnVictory { life, minimum },
            },
        });
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
                        == AttributeAffectedV1::Life))
            || has_victory_or_defeat_opponent_life_shape(definition);
    // Equalizer post-round work is a closed grammar plus two identities. Captured Copy can
    // put either reviewed id in either source slot, but every malformed record and adjacent
    // Equalizer Life or Pillz form - a wrong slot, a cap, a compound, the complete shape under
    // other text - must reject when selected.
    let unadmitted_equalizer_opponent_life = matches!(source.id, 1415 | 4458)
        || (source.description.contains("Equalizer")
            && (source.description.contains("Life")
                || source.description.contains("Pillz")
                || matches!(
                    definition.structured_input().attribute_affected,
                    AttributeAffectedV1::Life
                        | AttributeAffectedV1::Pillz
                        | AttributeAffectedV1::LifeAndPillz
                )))
        || has_equalizer_post_round_shape(definition);
    // A near-miss of the admitted family is a selected hazard: either the literal grammar
    // names Victory Life but its structure is wrong, or the complete reviewed structure
    // is present under malformed text. Its `Courage:` form takes the boundary with it. Other
    // Life families keep the diagnostic's existing visible-but-disabled behavior until their
    // own slices are implemented.
    let unadmitted_victory_life = ((source.description.starts_with('+')
        || source.description.starts_with("Courage: +"))
        && source.description.ends_with(" Life")
        && definition.structured_input().attribute_affected == AttributeAffectedV1::Life)
        || has_victory_life_shape(definition);
    // Plain Victory Pillz is the same kind of near-miss boundary: the literal `+N Pillz`
    // text over a wrong structure or slot, or the complete reviewed structure under other
    // text, rejects when selected. Its `Confidence:` form joined the grammar in revision 36
    // and its `Courage:` form in revision 55, and each takes the boundary with it, so either
    // text over a wrong structure is a hazard too. The other prefixed forms (`Stop:`,
    // `Growth:`, `Killshot:`, `Perfect:`, `Defeat:`, `Revenge:`) and the capped
    // `+3 Pillz Max. 9` differ structurally and keep their visible-but-disabled records;
    // `Brawl:`, `Support:` and `Equalizer:` have their own post-round boundaries.
    let unadmitted_victory_pillz = ((source.description.starts_with('+')
        || source.description.starts_with("Confidence: +")
        || source.description.starts_with("Courage: +"))
        && source.description.ends_with(" Pillz")
        && definition.structured_input().attribute_affected == AttributeAffectedV1::Pillz)
        || has_victory_pillz_shape(definition);
    // The opposing reduction has the same two-sided boundary. `Stop:`, `Growth:`,
    // `Bet > N Pillz:` and clan-gated forms differ structurally and stay
    // visible-but-disabled, `Brawl:` is its own grammar below, and the dotted `Opp. Pillz`
    // compounds are other grammars entirely.
    let unadmitted_victory_opponent_pillz = (source.description.starts_with('-')
        && source.description.contains("Opp Pillz")
        && definition.structured_input().attribute_affected == AttributeAffectedV1::Pillz)
        || has_victory_opponent_pillz_shape(definition);
    // Its losing-side sibling prints the dotted spelling under a `Defeat:` prefix. A near
    // miss - the clan-gated `4673`, or the complete shape under other text - stays a
    // selected hazard rather than an inert disabled source.
    let unadmitted_defeat_opponent_pillz = (source.description.starts_with("Defeat: -")
        && source.description.contains("Opp. Pillz")
        && definition.structured_input().attribute_affected == AttributeAffectedV1::Pillz)
        || has_defeat_opponent_pillz_shape(definition);
    // The Pillz-per-Damage conversion is a closed grammar over one special action: any
    // record that converts Damage to Pillz, prints the text, or carries the complete shape
    // under another hand-slot prefix rejects when selected.
    let unadmitted_victory_life_per_opponent_damage =
        source.description.ends_with("Life Per Opp. Damage")
            || definition.structured_input().special_action
                == SpecialActionV1::ConvertOpponentDamageToLife;
    let unadmitted_victory_pillz_per_damage = source.description.ends_with("Pillz Per Damage")
        || definition.structured_input().special_action == SpecialActionV1::ConvertDamageToPillz
        || has_victory_pillz_per_damage_shape(definition);
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
    // The plain reduction now has the same two-sided boundary every other post-round
    // grammar has: the literal `-N Opp. Life Min M` text over a wrong slot or structure, or
    // the complete reviewed structure under other text - which is what `Night: -2 Opp. Life
    // Min 0` is - rejects when selected. The identity-locked members stay listed so a
    // malformed record or a wrong source slot cannot quietly become an inert no-op.
    // The Courage members joined the identity table in semantic revision 39; Growth 1730
    // still differs in a structured field and keeps its existing Disabled record.
    let unadmitted_victory_opponent_life = matches!(
        source.id,
        680 | 3016 | 3314 | 4301 | 4531 | 4532 | 4533 | 4708
    ) || source.description == "-2 Opp. Life Min 2"
        || (source.description.starts_with('-')
            && source.description.contains("Opp. Life")
            && input.attribute_affected == AttributeAffectedV1::Life)
        || has_victory_opponent_life_shape(definition);
    // Killshot's boundary is its own: the plain Victory clause above cannot reach it,
    // because the printed text does not start with `-` and the reviewed Victory shape
    // demands a won round where this one asks `sureshot`. Without this clause a malformed
    // Killshot record or a wrong source slot would quietly become an inert disabled source
    // instead of a selected hazard.
    let unadmitted_killshot_opponent_life = (source.description.starts_with("Killshot: -")
        && source.description.contains("Opp. Life")
        && input.attribute_affected == AttributeAffectedV1::Life)
        || has_killshot_opponent_life_shape(definition);
    // Xantiax's boundary is its own: the printed text over a wrong slot or structure, or
    // the complete both-sides shape under other text. Nothing else admitted reaches both
    // players at once, so any other record with that shape is a hazard rather than a
    // disabled no-op.
    let unadmitted_both_players_life_reduction = source.description.starts_with("Xantiax")
        || has_both_players_life_reduction_shape(definition);
    // The both-players Victory Or Defeat gains: the printed `Players` text over a wrong
    // slot or structure, or the complete shape under other text, rejects when selected.
    let unadmitted_both_players_gain = (source.description.starts_with("Victory Or Defeat")
        && source.description.contains("Players"))
        || has_victory_or_defeat_both_players_gain_shape(definition);
    // The post-round Brawl grammars have the same two-sided boundary: a `Brawl:` text over a
    // resource it could pay - a wrong slot, a wrong structure, numbers the text disagrees
    // with - or the complete anti-support shape under other text rejects when selected.
    // The combat-stat `Brawl:` forms name Power, Damage or Attack and are not reached.
    let unadmitted_brawl_post_round = (source.description.starts_with("Brawl: ")
        && matches!(
            input.attribute_affected,
            AttributeAffectedV1::Life
                | AttributeAffectedV1::Pillz
                | AttributeAffectedV1::LifeAndPillz
        ))
        || has_brawl_post_round_shape(definition);
    // The post-round Support grammars have the same boundary: a `Support:` text over a Life
    // or Pillz record that is not a permanent - a wrong slot, a wrong structure, numbers the
    // text disagrees with - or the complete Support shape under other text rejects when
    // selected; before revision 55 these were inert disabled sources in replay. `Support:
    // Dope` is a Pillz permanent with a family of its own and keeps its record, and the
    // combat-stat Support forms name Power, Damage or Attack and are not reached.
    let unadmitted_support_post_round = (source.description.starts_with("Support: ")
        && !input.is_permanent
        && matches!(
            input.attribute_affected,
            AttributeAffectedV1::Life
                | AttributeAffectedV1::Pillz
                | AttributeAffectedV1::LifeAndPillz
        ))
        || has_support_post_round_shape(definition);
    // `Stop:` fires on the owner's own ability being stopped, which the projection admits
    // only over a numeric body and only where nothing opposite can stop it. Any other
    // `Stop:` record - the Pillz forms, a malformed body, the inverted flag under other
    // text - rejects when selected rather than acting as an inert disabled source.
    let unadmitted_stop_triggered =
        source.description.starts_with("Stop: ") || definition.structured_input().is_inverted;
    // `Defeat: +N Pillz` over a wrong slot or structure, or its complete shape under other
    // text, rejects when selected; before revision 52 it fell through to an inert source.
    let unadmitted_defeat_pillz = (source.description.starts_with("Defeat: +")
        && definition.structured_input().attribute_affected == AttributeAffectedV1::Pillz)
        || has_defeat_pillz_shape(definition);
    // A `Growth:`/`Degrowth:` text over a Life or Pillz record that is not a permanent, or
    // the complete round-scaled shape under other text, rejects when selected. Before
    // revision 53 these were inert disabled sources in replay.
    let unadmitted_round_scaled_post_round = ((source.description.starts_with("Growth: ")
        || source.description.starts_with("Degrowth: "))
        && !definition.structured_input().is_permanent
        && matches!(
            definition.structured_input().attribute_affected,
            AttributeAffectedV1::Life
                | AttributeAffectedV1::Pillz
                | AttributeAffectedV1::LifeAndPillz
        ))
        || has_round_scaled_post_round_shape(definition);
    // `Bet > N Pillz:` over a Life or Pillz record, or the complete gated Victory shape
    // under other text, rejects when selected; before the gate was admitted these were
    // inert disabled sources (the Zenith bonus `4657` among them).
    let unadmitted_bet_gated_post_round = ((source.description.starts_with("Bet > ")
        || source.description.starts_with("Bet < "))
        && matches!(
            input.attribute_affected,
            AttributeAffectedV1::Life
                | AttributeAffectedV1::Pillz
                | AttributeAffectedV1::LifeAndPillz
        ))
        || has_bet_gated_post_round_shape(definition);
    let unadmitted_komboka_victory_pillz_and_life = source.id == 1714
        || source.description == "+1 Pillz And Life"
        || (input.side_affected == crate::effect_registry::AffectedSideV1::Player
            && input.attribute_affected == AttributeAffectedV1::LifeAndPillz
            && input.attribute_action == AttributeActionV1::Increase);
    // A near-miss of an admitted permanent grammar is a selected hazard: its plain text in
    // the wrong slot or over a malformed structure, or its complete permanent shape under
    // text whose numbers or grammar disagree with it - which is what `Revenge:` Poison was
    // before revision 32 admitted it. `Growth:` and `Unison :` Poison were once described
    // here as the same case, but they are not: Growth carries `isOverdrive` and Unison the
    // clan-mates link, so no admitted shape reaches them and they get a clause of their own
    // at the end of this one rather than falling through to an inert disabled source.
    // `Defeat`, `Killshot`, `Symmetry`, `Asymmetry`, `Perfect`, `Backlash`, Victory-or-Defeat
    // and clan-gated forms differ in their structured fields and keep the visible-but-
    // disabled record every other permanent has.
    let unadmitted_heal_life = source.description.starts_with("Heal ")
        || source.description.starts_with("Regen ")
        || source.description.starts_with("Poison ")
        || source.description.starts_with("Toxin ")
        || has_heal_life_on_victory_shape(definition)
        || has_regen_life_on_victory_shape(definition)
        || has_poison_opponent_life_on_victory_shape(definition)
        || has_toxin_opponent_life_on_victory_shape(definition)
        // The losing-side latch has the same two-sided boundary: its printed text over a
        // wrong slot or structure, or the complete reviewed structure under other text.
        // Without this a malformed `Defeat: Poison` record would become an inert disabled
        // source rather than a selected hazard.
        || source.description.starts_with("Defeat: Poison ")
        || has_poison_opponent_life_on_defeat_shape(definition)
        // `Unison :` and `Growth:` permanents carry the clan-mates link or `isOverdrive`
        // beside the plain permanent structure, so none of the shapes above reach them;
        // without this they would be inert disabled sources whose latch replay drops.
        || ((source.description.starts_with("Unison") || source.description.starts_with("Growth"))
            && input.is_permanent
            && input.attribute_affected == AttributeAffectedV1::Life);
    let reason = if attempted_control {
        CombatStatDisabledReasonV1::UnsupportedPromisedControl { registry_reasons }
    } else if selected_hazard {
        CombatStatDisabledReasonV1::UnsupportedSelectedHazard { registry_reasons }
    } else if unadmitted_victory_or_defeat
        || unadmitted_victory_or_defeat_life
        || unadmitted_equalizer_opponent_life
        || unadmitted_victory_life
        || unadmitted_victory_pillz
        || unadmitted_victory_opponent_pillz
        || unadmitted_defeat_opponent_pillz
        || unadmitted_victory_pillz_per_damage
        || unadmitted_victory_life_per_opponent_damage
        || unadmitted_defeat_life
        || unadmitted_reanimate_life
        || unadmitted_argos_defeat_capped_pillz
        || unadmitted_anita_courage_damage_to_life
        || unadmitted_victory_opponent_life
        || unadmitted_killshot_opponent_life
        || unadmitted_komboka_victory_pillz_and_life
        || unadmitted_both_players_life_reduction
        || unadmitted_both_players_gain
        || unadmitted_brawl_post_round
        || unadmitted_support_post_round
        || unadmitted_stop_triggered
        || unadmitted_defeat_pillz
        || unadmitted_round_scaled_post_round
        || unadmitted_bet_gated_post_round
        || unadmitted_heal_life
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
        || unadmitted_victory_pillz
        || unadmitted_victory_opponent_pillz
        || unadmitted_defeat_opponent_pillz
        || unadmitted_victory_pillz_per_damage
        || unadmitted_victory_life_per_opponent_damage
        || unadmitted_defeat_life
        || unadmitted_reanimate_life
        || unadmitted_argos_defeat_capped_pillz
        || unadmitted_anita_courage_damage_to_life
        || unadmitted_victory_opponent_life
        || unadmitted_killshot_opponent_life
        || unadmitted_komboka_victory_pillz_and_life
        || unadmitted_both_players_life_reduction
        || unadmitted_both_players_gain
        || unadmitted_brawl_post_round
        || unadmitted_support_post_round
        || unadmitted_stop_triggered
        || unadmitted_defeat_pillz
        || unadmitted_round_scaled_post_round
        || unadmitted_bet_gated_post_round
        || unadmitted_heal_life
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
        CombatStatSourcePlanV1::Execute { effect, .. } if effect_reads_support_count(effect)
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
