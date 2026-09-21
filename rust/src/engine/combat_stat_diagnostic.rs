//! Explicit ordinary combat-stat diagnostic projection.
//!
//! This module owns compact effect plans and hot-path resolution. Capture-specific
//! preparation and rich disposition metadata live in the replay diagnostic module.

use super::combat_resolution::{
    prepare_combat_resolution_with_post_round, CombatResolutionArithmeticStage,
    CombatResolutionError, PreparedCombatResolution, ResolutionCardPlan, ResolutionSourcePlan,
};
use super::combat_stat_compiler::{
    anita_courage_damage_to_life_identity_matches, argos_defeat_capped_pillz_identity_matches,
    equalizer_opponent_life_on_victory_identity_matches,
    komboka_victory_pillz_and_life_identity_matches, permanent_predicate_admitted,
    victory_opponent_life_identity_matches, victory_opponent_life_predicate,
    victory_or_defeat_pillz_identity_matches,
};
use super::{
    BaseRulesError, BaseRulesGame, BaseRulesMatchSpec, BaseRulesPosition, BaseRulesRoundInput,
    BaseRulesRoundReport, BaseRulesUndo, ByPlayer, DiagnosticAffectedSideV1,
    DiagnosticCombatEffectV1, DiagnosticCombatStatV1, DiagnosticMagnitudeV1,
    DiagnosticStatOperationV1, HandSlot, LatchedEffectV1, PlayerId, PostRoundEffect,
    PostRoundSourceEffect, ValidatedSelection, HAND_SIZE,
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
    /// Scaled by the opposing selected card's resolved Damage, before Fury.
    OpponentDamage,
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
    GainOnePillzAndLifeOnVictory,
    GainTwoPillzOnDefeatMaxEleven,
    /// Anita's identity-locked Courage conversion, whose runtime magnitude is the final
    /// resolved damage dealt by its owner.
    GainLifeEqualToFinalDamageOnCourageVictory,
    GainLifeOnVictory {
        life: u16,
    },
    /// Plain `+N Pillz`: the winner's own Pillz rise by the printed amount. Admitted by
    /// exact text and shape from the Ability slot.
    GainPillzOnVictory {
        pillz: u16,
    },
    /// Plain `-N Opp Pillz. Min M`: the winner takes `pillz` from the opposing player, never
    /// below `minimum`. Admitted by exact text and shape from the Ability slot.
    ReduceOpponentPillzOnVictory {
        pillz: u16,
        minimum: u16,
    },
    /// `Defeat: -N Opp. Pillz, Min M`: the losing owner takes `pillz` from the opposing
    /// player, never below `minimum`. The Victory reduction's losing-side sibling.
    ReduceOpponentPillzOnDefeat {
        pillz: u16,
        minimum: u16,
    },
    /// `+1 Pillz Per Damage` and its `Symmetry:` form: the winner's own Pillz rise by the
    /// final resolved Damage its card dealt. The predicate carries the hand-slot condition.
    GainPillzEqualToFinalDamageOnVictory,
    /// `+N Life Per Damage`, its capped `Max. M` form and its `Revenge:`/`Confidence:`
    /// forms: the winner's own Life rises by `life_per_damage` for every point of final
    /// resolved Damage, never past `maximum` when that is non-zero.
    GainLifePerFinalDamageOnVictory {
        life_per_damage: u16,
        maximum: u16,
    },
    GainLifeOnDefeat {
        life: u16,
    },
    ReanimateLife {
        life: u16,
    },
    GainLifeOnVictoryOrDefeat {
        life: u16,
    },
    ReduceOpponentLifeOnVictoryOrDefeat {
        life: u16,
        minimum: u16,
    },
    ReduceOpponentLifeOnVictoryPerOpponentStars {
        per_star: u16,
        minimum: u16,
    },
    /// The two reviewed unconditional Victory opponent-Life reductions.
    ReduceOpponentLifeOnVictory {
        life: u16,
        minimum: u16,
    },
    /// The losing-side sibling: the owner lost the round, so the opposing player's Life is
    /// reduced, bounded below by `minimum`.
    ReduceOpponentLifeOnDefeat {
        life: u16,
        minimum: u16,
    },
    /// The `sureshot` sibling: the owner's final attack was at least double the opposing
    /// one, so the opposing player's Life is reduced, bounded below by `minimum`. Winning
    /// the round is neither required nor sufficient.
    ReduceOpponentLifeOnKillshot {
        life: u16,
        minimum: u16,
    },
    /// `Xantiax: -N Life, Min. M`: the round's outcome is irrelevant and so is which side
    /// owns the source - both players lose `life`, neither below `minimum`.
    ReduceBothPlayersLife {
        life: u16,
        minimum: u16,
    },
    /// `Heal N Max. M`: won rounds latch it, every later round pays `life` while the owner
    /// is below `maximum`. Admitted by exact text and shape from the Ability slot.
    HealLifeOnVictory {
        life: u16,
        maximum: u16,
    },
    /// `Regen N, Max. M`: Heal that also pays in its latching round.
    RegenLifeOnVictory {
        life: u16,
        maximum: u16,
    },
    /// `Poison N, Min M`: from the round after the latch, the opposing player loses `life`
    /// while above `minimum`. Admitted from either slot; Freaks print it as their bonus.
    PoisonOpponentLifeOnVictory {
        life: u16,
        minimum: u16,
    },
    /// `Toxin N, Min M`: Poison that also pays in its latching round. Ability slot only.
    ToxinOpponentLifeOnVictory {
        life: u16,
        minimum: u16,
    },
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
    StopOpponentAbility,
    StopOpponentBonus,
    CancelOpponentCombatStatModifiers {
        stat: CombatStatAttributeV1,
    },
    /// The owner's own stat cannot be reduced by the opposing selected card. It is not a
    /// modifier: it removes nothing and adds nothing, it only refuses opposing decreases.
    ProtectOwnCombatStat {
        stat: CombatStatAttributeV1,
    },
    /// The owner's own Ability survives an opposing Stop, provided this source itself
    /// survived the Stop resolution that round.
    ProtectOwnAbility,
    /// The owner's own Bonus survives an opposing Stop, under the same condition.
    ProtectOwnBonus,
    /// The owner's own stat is replaced by the opposing selected card's printed value,
    /// before any increase of its own and before any opposing reduction.
    CopyOpponentPrintedCombatStat {
        stat: CombatStatAttributeV1,
    },
    /// Fixed non-stat post-round work. Identity and source are checked at plan
    /// construction; its values are intentionally not caller-configurable.
    RecoverPaidPillzOnDefeat,
    /// Fixed end-of-round resource work. It is neither a combat modifier nor configurable
    /// public data: a direct plan must use one exact audited source/id pair.
    GainOnePillzOnVictoryOrDefeat,
    /// Komboka's exact composite clan-bonus Victory work.  The two additions are coupled
    /// in the private commit plan so direct callers cannot split or reconfigure them.
    GainOnePillzAndLifeOnVictory,
    /// Argos' fixed surviving-Defeat gain, applied after the clan bonus and capped at 11
    /// without lowering a value which is already at or above that cap.
    GainTwoPillzOnDefeatMaxEleven,
    /// Anita's identity-locked Courage conversion, whose runtime magnitude is the final
    /// resolved damage dealt by its owner.
    GainLifeEqualToFinalDamageOnCourageVictory,
    /// Generic fixed Victory Life, admitted only by the cold structured compiler.  The
    /// hot plan retains its exact positive magnitude but no strings or registry access.
    GainLifeOnVictory {
        life: u16,
    },
    /// Plain Victory Pillz, admitted only by the cold structured compiler from the Ability
    /// slot. The hot plan retains its exact positive magnitude and nothing else.
    GainPillzOnVictory {
        pillz: u16,
    },
    /// Victory-only reduction of the opposing player's Pillz with a fixed magnitude and lower
    /// bound, admitted only by the cold structured compiler from the Ability slot.
    ReduceOpponentPillzOnVictory {
        pillz: u16,
        minimum: u16,
    },
    /// Defeat-only reduction of the opposing player's Pillz with a fixed magnitude and lower
    /// bound, admitted only by the cold structured compiler from the Ability slot.
    ReduceOpponentPillzOnDefeat {
        pillz: u16,
        minimum: u16,
    },
    /// The winner's own Pillz rise by its card's final resolved Damage. Unconditional or
    /// under the Symmetry hand-slot predicate; Ability slot only.
    GainPillzEqualToFinalDamageOnVictory,
    /// The winner's own Life rises by `life_per_damage` per point of final resolved Damage,
    /// bounded above by `maximum` when that is non-zero. Unconditional or under a
    /// previous-round predicate; Ability slot only, and a cap only without a predicate.
    GainLifePerFinalDamageOnVictory {
        life_per_damage: u16,
        maximum: u16,
    },
    /// Ordinary Defeat Life applies only after a surviving loss. The hot path retains the
    /// exact positive magnitude but no strings or registry access.
    GainLifeOnDefeat {
        life: u16,
    },
    /// Reanimate is the explicit Defeat-Life exception permitted to revive its owner from
    /// zero before terminal status is calculated.
    ReanimateLife {
        life: u16,
    },
    /// Gain the owner's life after either round outcome, provided they survived damage.
    GainLifeOnVictoryOrDefeat {
        life: u16,
    },
    /// Reduce the opposing player's life after either round outcome, bounded below by `minimum`.
    ReduceOpponentLifeOnVictoryOrDefeat {
        life: u16,
        minimum: u16,
    },
    /// Victory-only opponent-Life reduction whose magnitude is bound from the selected
    /// opposing card's stars after Stop liveness has been resolved.
    ReduceOpponentLifeOnVictoryPerOpponentStars {
        per_star: u16,
        minimum: u16,
    },
    /// Unconditional Victory-only opponent-Life reduction with a fixed magnitude and
    /// lower bound, admitted solely for the two reviewed identities.
    ReduceOpponentLifeOnVictory {
        life: u16,
        minimum: u16,
    },
    /// Defeat-only opponent-Life reduction: the owner having lost is the trigger, and a
    /// target already at or below `minimum` is left alone.
    ReduceOpponentLifeOnDefeat {
        life: u16,
        minimum: u16,
    },
    /// Killshot opponent-Life reduction: doubling the opposing final attack is the trigger,
    /// and a target already at or below `minimum` is left alone.
    ReduceOpponentLifeOnKillshot {
        life: u16,
        minimum: u16,
    },
    /// `Xantiax: -N Life, Min. M`: the only admitted post-round effect with no outcome
    /// channel and no owning side. Both players lose `life`, neither taken below
    /// `minimum`, whoever won and whether or not the owner is already out.
    ReduceBothPlayersLife {
        life: u16,
        minimum: u16,
    },
    /// The projection's first repeating effect. A won round latches it into the owner's
    /// position and pays nothing itself; the base engine then pays `life` at the end of
    /// every later round while the owner is living and below `maximum`, whatever card is
    /// played and whoever wins. Stop liveness is judged once, in the latching round.
    HealLifeOnVictory {
        life: u16,
        maximum: u16,
    },
    /// Heal's immediate sibling: the latching round pays too.
    RegenLifeOnVictory {
        life: u16,
        maximum: u16,
    },
    /// The opposing player loses `life` at the end of every round after the latch while
    /// above `minimum`, including a round in which the owner is knocked out.
    PoisonOpponentLifeOnVictory {
        life: u16,
        minimum: u16,
    },
    /// Poison's immediate sibling. With Min 0 the repeat itself can end the match.
    ToxinOpponentLifeOnVictory {
        life: u16,
        minimum: u16,
    },
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
    /// Copy. The source has no effect of its own: at round resolution it adopts the opposing
    /// selected card's corresponding immutable source plan, keeping this card's own slot
    /// kind for Stop liveness and its own Support context. The opposing plan is already
    /// materialized, so resolution stays a plain index.
    ///
    /// `predicate` gates the adoption itself and is `Always` for an unconditional Copy. It
    /// is resolved before the round is prepared, exactly like a combat-stat predicate, so a
    /// conditional Copy costs nothing extra in the hot path. The adopted plan keeps its own
    /// predicate afterwards: copying a Confidence effect does not inherit the opponent's
    /// history, it has to satisfy the copier's.
    CopyOpponentSource {
        source_id: u32,
        copied: CopiedSourceKindV1,
        predicate: CombatStatPredicateV1,
    },
    Disabled {
        source_id: u32,
    },
    RejectIfSelected {
        source_id: u32,
    },
}

/// Which of the opposing selected card's two sources an unconditional Copy adopts.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CopiedSourceKindV1 {
    Ability,
    Bonus,
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
    KombokaVictoryPillzAndLifeClan,
    KombokaVictoryPillzAndLifeEffect,
    KombokaVictoryPillzAndLifeIdentity,
    KombokaVictoryPillzAndLifePredicate,
    ArgosDefeatCappedPillzIdentity,
    ArgosDefeatCappedPillzPredicate,
    AnitaCourageDamageToLifeCard,
    AnitaCourageDamageToLifeEffect,
    AnitaCourageDamageToLifeIdentity,
    AnitaCourageDamageToLifePredicate,
    HealLifeSource,
    HealLifeMagnitude,
    HealLifePredicate,
    /// Regen and Toxin are card abilities only; Poison may also be the Freaks bonus.
    PermanentLifeSource,
    PermanentLifeMagnitude,
    PermanentLifePredicate,
    CopyOpponentSourceIdentity,
    CopyOpponentSourceTarget,
    VictoryOpponentLifeIdentity,
    VictoryOpponentLifeMagnitude,
    VictoryOpponentLifePredicate,
    VictoryLifeMagnitude,
    VictoryLifePredicate,
    VictoryPillzSource,
    VictoryPillzMagnitude,
    VictoryPillzPredicate,
    VictoryOpponentPillzSource,
    VictoryOpponentPillzMagnitude,
    VictoryOpponentPillzPredicate,
    DefeatOpponentPillzSource,
    DefeatOpponentPillzMagnitude,
    DefeatOpponentPillzPredicate,
    VictoryPillzPerDamageSource,
    VictoryPillzPerDamagePredicate,
    VictoryLifePerDamageSource,
    VictoryLifePerDamageMagnitude,
    VictoryLifePerDamagePredicate,
    DefeatLifeSource,
    DefeatLifeMagnitude,
    DefeatLifePredicate,
    ReanimateLifeSource,
    ReanimateLifeMagnitude,
    ReanimateLifePredicate,
    VictoryOrDefeatLifeIdentity,
    VictoryOrDefeatLifeEffect,
    VictoryOrDefeatLifePredicate,
    EqualizerOpponentLifeIdentity,
    EqualizerOpponentLifeEffect,
    EqualizerOpponentLifePredicate,
    ReprisalStopOpponentAbilityCard,
    ReprisalStopOpponentAbilityEffect,
    ReprisalStopOpponentAbilityIdentity,
    ReprisalStopOpponentAbilityPredicate,
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
                    spec.cards[player][slot.index()].key,
                    spec.cards[player][slot.index()].effective_clan_id,
                    CombatStatEffectSourceV1::Ability,
                    spec.cards[player][slot.index()].ability,
                )?;
                validate_combat_stat_source_plan(
                    player,
                    slot,
                    spec.cards[player][slot.index()].key,
                    spec.cards[player][slot.index()].effective_clan_id,
                    CombatStatEffectSourceV1::Bonus,
                    spec.cards[player][slot.index()].bonus,
                )?;
                validate_source_bonus_context(player, slot, &spec.cards[player])?;
                validate_ability_support_context(player, slot, &spec.cards[player])?;
                validate_copy_targets(
                    player,
                    slot,
                    &spec.cards[player],
                    &spec.cards[player.other()],
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
    // A Copy ability cannot know which opposing source it will adopt, and that source may
    // carry Support magnitude. It therefore always carries this card's own effective-clan
    // context; an adopted effect without Support magnitude simply ignores the count.
    let expected = if matches!(plan, CombatStatSourcePlanV1::CopyOpponentSource { .. }) {
        effective_clan_character_count(hand_slot, cards)
    } else {
        expected
    };
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

/// A solver must be total over every legal selection, so a Copy is admissible only when
/// every opposing card it could face already carries a concrete adoptable plan. A Copy of
/// a Copy, of a disabled source, or of a selected hazard is rejected for the whole match
/// rather than deferred to the round that would have to resolve it.
///
/// A condition does not soften this. The predicate depends on the round, not on the draw,
/// so every conditional Copy has some legal line in which it does adopt, and admitting one
/// whose target is unresolvable would only move the failure to that line.
fn validate_copy_targets(
    player: PlayerId,
    hand_slot: HandSlot,
    own: &[CombatStatCardPlanV1; HAND_SIZE],
    opponent: &[CombatStatCardPlanV1; HAND_SIZE],
) -> Result<(), CombatStatPlanErrorV1> {
    for (source, plan) in [
        (
            CombatStatEffectSourceV1::Ability,
            own[hand_slot.index()].ability,
        ),
        (
            CombatStatEffectSourceV1::Bonus,
            own[hand_slot.index()].bonus,
        ),
    ] {
        let CombatStatSourcePlanV1::CopyOpponentSource {
            source_id, copied, ..
        } = plan
        else {
            continue;
        };
        for opposing in opponent.iter() {
            let target = match copied {
                CopiedSourceKindV1::Ability => opposing.ability,
                CopiedSourceKindV1::Bonus => opposing.bonus,
            };
            if !matches!(
                target,
                CombatStatSourcePlanV1::Absent | CombatStatSourcePlanV1::Execute { .. }
            ) {
                return Err(invalid_combat_stat_execute(
                    player,
                    hand_slot,
                    source,
                    source_id,
                    InvalidCombatStatPlanReasonV1::CopyOpponentSourceTarget,
                ));
            }
        }
    }
    Ok(())
}

fn source_plan_id(plan: CombatStatSourcePlanV1) -> Option<u32> {
    match plan {
        CombatStatSourcePlanV1::Absent => None,
        CombatStatSourcePlanV1::Execute { source_id, .. }
        | CombatStatSourcePlanV1::CopyOpponentSource { source_id, .. }
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

/// The VOD-Life slice is deliberately a closed set of reviewed registry results. These
/// compact plans also represent server-observed post-Copy results, so source kind and card
/// key are intentionally *not* part of this hot-plan boundary; the catalog constructor is
/// responsible for keeping canonical printed cards narrower.
fn victory_or_defeat_life_id_is_reserved(source_id: u32) -> bool {
    matches!(source_id, 1396 | 1628 | 2944 | 2992 | 5799 | 5802 | 5835)
}

fn equalizer_opponent_life_id_is_reserved(source_id: u32) -> bool {
    matches!(source_id, 1415 | 4458)
}

/// The Victory opponent-Life ids that stay identity-locked whatever a caller claims: the
/// one clan Bonus and the three conditional abilities. Their magnitudes cannot be smuggled
/// onto another id, and no other magnitude can ride theirs. Every remaining printed ability
/// goes through the plain grammar below.
fn victory_opponent_life_id_is_reserved(source_id: u32) -> bool {
    matches!(
        source_id,
        680 | 3016 | 3314 | 4301 | 4531 | 4532 | 4533 | 4708
    )
}

/// The conditional sibling this slice deliberately leaves out. `1730` is a round-scaled
/// magnitude rather than a predicate, so it may not ride the plain grammar however a caller
/// labels it. Its two paying rounds are good evidence for a later slice, not for this one.
fn victory_opponent_life_id_is_deferred(source_id: u32) -> bool {
    matches!(source_id, 1730)
}

fn victory_opponent_life_effect_matches(source_id: u32, effect: CombatStatEffectV1) -> bool {
    matches!(
        (source_id, effect),
        (
            680,
            CombatStatEffectV1::ReduceOpponentLifeOnVictory {
                life: 2,
                minimum: 2
            }
        ) | (
            4708,
            CombatStatEffectV1::ReduceOpponentLifeOnVictory {
                life: 4,
                minimum: 0
            }
        ) | (
            3016 | 4301 | 4533,
            CombatStatEffectV1::ReduceOpponentLifeOnVictory {
                life: 3,
                minimum: 0
            }
        ) | (
            3314,
            CombatStatEffectV1::ReduceOpponentLifeOnVictory {
                life: 1,
                minimum: 0
            }
        ) | (
            4531 | 4532,
            CombatStatEffectV1::ReduceOpponentLifeOnVictory {
                life: 3,
                minimum: 1
            }
        )
    )
}

const ANITA_COURAGE_DAMAGE_TO_LIFE_CARD: CardKey = CardKey { id: 448, level: 3 };

fn equalizer_opponent_life_effect_matches(source_id: u32, effect: CombatStatEffectV1) -> bool {
    matches!(
        (source_id, effect),
        (
            1415 | 4458,
            CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars {
                per_star: 1,
                minimum: 2
            }
        )
    )
}

fn victory_or_defeat_life_effect_matches(source_id: u32, effect: CombatStatEffectV1) -> bool {
    matches!(
        (source_id, effect),
        (
            1396 | 2992 | 5799 | 5835,
            CombatStatEffectV1::GainLifeOnVictoryOrDefeat { life: 1 }
        ) | (
            2944 | 5802,
            CombatStatEffectV1::GainLifeOnVictoryOrDefeat { life: 2 }
        ) | (
            1628,
            CombatStatEffectV1::ReduceOpponentLifeOnVictoryOrDefeat {
                life: 1,
                minimum: 1
            }
        )
    )
}

fn validate_combat_stat_source_plan(
    player: PlayerId,
    hand_slot: HandSlot,
    card_key: CardKey,
    effective_clan_id: u32,
    source: CombatStatEffectSourceV1,
    plan: CombatStatSourcePlanV1,
) -> Result<(), CombatStatPlanErrorV1> {
    // Only the two reviewed unconditional Copy identities exist, and each is locked to the
    // opposing source it adopts: 764 takes the Bonus, 2918 takes the Ability.
    // Copy is generic-by-grammar, like Victory Life: exact description and structured
    // shape are the cold compiler's authority, so the string-free plan can only require a
    // real source identity here. Totality against every opposing card is checked
    // separately, once both hands are known.
    if let CombatStatSourcePlanV1::CopyOpponentSource { source_id, .. } = plan {
        return if source_id == 0 {
            Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::CopyOpponentSourceIdentity,
            ))
        } else {
            Ok(())
        };
    }
    let CombatStatSourcePlanV1::Execute {
        source_id,
        predicate,
        effect,
    } = plan
    else {
        return Ok(());
    };
    // Anita's conversion remains a single printed Ability identity.  It cannot be
    // borrowed by another card, source slot, predicate, or dynamic Copy provenance.
    if source_id == 274 {
        if !anita_courage_damage_to_life_identity_matches(source, source_id) {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::AnitaCourageDamageToLifeIdentity,
            ));
        }
        if card_key != ANITA_COURAGE_DAMAGE_TO_LIFE_CARD {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::AnitaCourageDamageToLifeCard,
            ));
        }
        if effect != CombatStatEffectV1::GainLifeEqualToFinalDamageOnCourageVictory {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::AnitaCourageDamageToLifeEffect,
            ));
        }
        if predicate != CombatStatPredicateV1::OwnerMovesFirst {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::AnitaCourageDamageToLifePredicate,
            ));
        }
        return Ok(());
    }
    if effect == CombatStatEffectV1::GainLifeEqualToFinalDamageOnCourageVictory {
        return Err(invalid_combat_stat_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidCombatStatPlanReasonV1::AnitaCourageDamageToLifeIdentity,
        ));
    }
    // Heal is generic by grammar, like Victory Life: the cold compiler's exact text and
    // shape are the authority, so the string-free plan can only require the Ability slot, a
    // positive magnitude below a positive cap, and no condition of its own.
    // The other three permanents follow the same generic-by-grammar rule. Poison is the one
    // permanent a clan prints as its bonus, so it alone is open to both slots.
    match effect {
        CombatStatEffectV1::RegenLifeOnVictory { life, maximum } => {
            if source != CombatStatEffectSourceV1::Ability {
                return Err(invalid_combat_stat_execute(
                    player,
                    hand_slot,
                    source,
                    source_id,
                    InvalidCombatStatPlanReasonV1::PermanentLifeSource,
                ));
            }
            if life == 0 || maximum <= life {
                return Err(invalid_combat_stat_execute(
                    player,
                    hand_slot,
                    source,
                    source_id,
                    InvalidCombatStatPlanReasonV1::PermanentLifeMagnitude,
                ));
            }
            if !permanent_predicate_admitted(predicate) {
                return Err(invalid_combat_stat_execute(
                    player,
                    hand_slot,
                    source,
                    source_id,
                    InvalidCombatStatPlanReasonV1::PermanentLifePredicate,
                ));
            }
            return Ok(());
        }
        CombatStatEffectV1::PoisonOpponentLifeOnVictory { life, .. }
        | CombatStatEffectV1::ToxinOpponentLifeOnVictory { life, .. } => {
            if source != CombatStatEffectSourceV1::Ability
                && !matches!(
                    effect,
                    CombatStatEffectV1::PoisonOpponentLifeOnVictory { .. }
                )
            {
                return Err(invalid_combat_stat_execute(
                    player,
                    hand_slot,
                    source,
                    source_id,
                    InvalidCombatStatPlanReasonV1::PermanentLifeSource,
                ));
            }
            if life == 0 {
                return Err(invalid_combat_stat_execute(
                    player,
                    hand_slot,
                    source,
                    source_id,
                    InvalidCombatStatPlanReasonV1::PermanentLifeMagnitude,
                ));
            }
            if !permanent_predicate_admitted(predicate) {
                return Err(invalid_combat_stat_execute(
                    player,
                    hand_slot,
                    source,
                    source_id,
                    InvalidCombatStatPlanReasonV1::PermanentLifePredicate,
                ));
            }
            return Ok(());
        }
        _ => {}
    }
    if let CombatStatEffectV1::HealLifeOnVictory { life, maximum } = effect {
        if source != CombatStatEffectSourceV1::Ability {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::HealLifeSource,
            ));
        }
        if life == 0 || maximum <= life {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::HealLifeMagnitude,
            ));
        }
        if !permanent_predicate_admitted(predicate) {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::HealLifePredicate,
            ));
        }
        return Ok(());
    }
    // Every reviewed Victory opponent-Life identity is source-kind, magnitude and predicate
    // locked, and the effect itself may not appear under any other id. A conditional member
    // carries exactly the one predicate its printed text names, so a plan cannot pair a
    // reviewed magnitude with some other condition.
    if victory_opponent_life_id_is_reserved(source_id) {
        if !victory_opponent_life_identity_matches(source, source_id) {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOpponentLifeIdentity,
            ));
        }
        if !victory_opponent_life_effect_matches(source_id, effect) {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOpponentLifeMagnitude,
            ));
        }
        if Some(predicate) != victory_opponent_life_predicate(source_id) {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOpponentLifePredicate,
            ));
        }
        return Ok(());
    }
    // Every other Victory opponent-Life reduction is the plain grammar: card abilities only,
    // a positive magnitude, and no condition beyond the outcome the engine already resolves.
    if let CombatStatEffectV1::ReduceOpponentLifeOnVictory { life, .. } = effect {
        if source != CombatStatEffectSourceV1::Ability
            || victory_opponent_life_id_is_deferred(source_id)
        {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOpponentLifeIdentity,
            ));
        }
        if life == 0 {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOpponentLifeMagnitude,
            ));
        }
        if predicate != CombatStatPredicateV1::Always {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOpponentLifePredicate,
            ));
        }
        return Ok(());
    }
    // Reserve every reviewed VOD-Life identity independently of its claimed source. A
    // caller cannot turn Scott's +1 into Uuber's reduction or smuggle an otherwise
    // plausible VOD-Life value through a generic plan. Captured Copy may legitimately
    // materialise either an Ability or Bonus result, so source kind remains open here.
    if victory_or_defeat_life_id_is_reserved(source_id) {
        if !victory_or_defeat_life_effect_matches(source_id, effect) {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOrDefeatLifeEffect,
            ));
        }
        if predicate != CombatStatPredicateV1::Always {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOrDefeatLifePredicate,
            ));
        }
        return Ok(());
    }
    if equalizer_opponent_life_id_is_reserved(source_id) {
        if !equalizer_opponent_life_on_victory_identity_matches(source, source_id)
            || !equalizer_opponent_life_effect_matches(source_id, effect)
        {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::EqualizerOpponentLifeEffect,
            ));
        }
        if predicate != CombatStatPredicateV1::Always {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::EqualizerOpponentLifePredicate,
            ));
        }
        return Ok(());
    }
    if matches!(
        effect,
        CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars { .. }
    ) {
        return Err(invalid_combat_stat_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidCombatStatPlanReasonV1::EqualizerOpponentLifeIdentity,
        ));
    }
    // The Victory-or-Defeat opposing reduction is the same plain grammar on the channel that
    // pays whatever the outcome; the own-Life gains stay a closed reviewed set.
    if let CombatStatEffectV1::ReduceOpponentLifeOnVictoryOrDefeat { life, .. } = effect {
        if source != CombatStatEffectSourceV1::Ability {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOrDefeatLifeIdentity,
            ));
        }
        if life == 0 {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOrDefeatLifeEffect,
            ));
        }
        if predicate != CombatStatPredicateV1::Always {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOrDefeatLifePredicate,
            ));
        }
        return Ok(());
    }
    if matches!(effect, CombatStatEffectV1::GainLifeOnVictoryOrDefeat { .. }) {
        return Err(invalid_combat_stat_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidCombatStatPlanReasonV1::VictoryOrDefeatLifeIdentity,
        ));
    }
    // These ids are reserved independently of the caller-provided source kind. They cannot
    // be repurposed as generic Bonus or numeric provenance tags to bypass Reprisal's strict
    // owner, effect, and defender-position contract.
    if reprisal_stop_opponent_ability_id_is_reserved(source_id) {
        if source != CombatStatEffectSourceV1::Ability {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::ReprisalStopOpponentAbilityIdentity,
            ));
        }
        if !reprisal_stop_opponent_ability_card_matches(source_id, card_key) {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::ReprisalStopOpponentAbilityCard,
            ));
        }
        if effect != CombatStatEffectV1::StopOpponentAbility {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::ReprisalStopOpponentAbilityEffect,
            ));
        }
        if predicate != CombatStatPredicateV1::OwnerMovesSecond {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::ReprisalStopOpponentAbilityPredicate,
            ));
        }
        return Ok(());
    }
    // Komboka's registry id is likewise reserved independently of the claimed source.
    // It denotes one composite Bonus-only Victory operation, never a generic numeric or
    // ordinary Life/Pillz provenance tag.
    if komboka_victory_pillz_and_life_id_is_reserved(source_id) {
        if source != CombatStatEffectSourceV1::Bonus {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::KombokaVictoryPillzAndLifeIdentity,
            ));
        }
        if effect != CombatStatEffectV1::GainOnePillzAndLifeOnVictory {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::KombokaVictoryPillzAndLifeEffect,
            ));
        }
        if predicate != CombatStatPredicateV1::Always {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::KombokaVictoryPillzAndLifePredicate,
            ));
        }
        if effective_clan_id != KOMBOKA_EFFECTIVE_CLAN_ID {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::KombokaVictoryPillzAndLifeClan,
            ));
        }
        return Ok(());
    }
    if effect == CombatStatEffectV1::StopOpponentAbility
        && predicate == CombatStatPredicateV1::OwnerMovesSecond
    {
        return Err(invalid_combat_stat_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidCombatStatPlanReasonV1::ReprisalStopOpponentAbilityIdentity,
        ));
    }
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
    if effect == CombatStatEffectV1::GainOnePillzAndLifeOnVictory {
        if !komboka_victory_pillz_and_life_identity_matches(source, source_id) {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::KombokaVictoryPillzAndLifeIdentity,
            ));
        }
        if predicate != CombatStatPredicateV1::Always {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::KombokaVictoryPillzAndLifePredicate,
            ));
        }
        return Ok(());
    }
    if effect == CombatStatEffectV1::GainTwoPillzOnDefeatMaxEleven {
        if !argos_defeat_capped_pillz_identity_matches(source, source_id) {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::ArgosDefeatCappedPillzIdentity,
            ));
        }
        if predicate != CombatStatPredicateV1::Always {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::ArgosDefeatCappedPillzPredicate,
            ));
        }
        return Ok(());
    }
    if let CombatStatEffectV1::GainLifeOnVictory { life } = effect {
        if life == 0 {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryLifeMagnitude,
            ));
        }
        // The plain grammar is unconditional; the two reviewed prefixed forms carry one
        // already-resolved predicate each, and both are card abilities only.
        if !matches!(
            predicate,
            CombatStatPredicateV1::Always
                | CombatStatPredicateV1::OwnerWonPreviousRound
                | CombatStatPredicateV1::SelectedHandSlotsDiffer
        ) || (predicate != CombatStatPredicateV1::Always
            && source != CombatStatEffectSourceV1::Ability)
        {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryLifePredicate,
            ));
        }
        return Ok(());
    }
    if let CombatStatEffectV1::GainPillzOnVictory { pillz } = effect {
        if source != CombatStatEffectSourceV1::Ability {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryPillzSource,
            ));
        }
        if pillz == 0 {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryPillzMagnitude,
            ));
        }
        // The plain grammar is unconditional; the one reviewed prefixed form carries the
        // previous-round predicate `Confidence:` names, and both are card abilities only.
        if !matches!(
            predicate,
            CombatStatPredicateV1::Always | CombatStatPredicateV1::OwnerWonPreviousRound
        ) {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryPillzPredicate,
            ));
        }
        return Ok(());
    }
    if let CombatStatEffectV1::ReduceOpponentPillzOnVictory { pillz, .. } = effect {
        if source != CombatStatEffectSourceV1::Ability {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOpponentPillzSource,
            ));
        }
        if pillz == 0 {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOpponentPillzMagnitude,
            ));
        }
        if predicate != CombatStatPredicateV1::Always {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOpponentPillzPredicate,
            ));
        }
        return Ok(());
    }
    if let CombatStatEffectV1::ReduceOpponentPillzOnDefeat { pillz, .. } = effect {
        if source != CombatStatEffectSourceV1::Ability {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::DefeatOpponentPillzSource,
            ));
        }
        if pillz == 0 {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::DefeatOpponentPillzMagnitude,
            ));
        }
        if predicate != CombatStatPredicateV1::Always {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::DefeatOpponentPillzPredicate,
            ));
        }
        return Ok(());
    }
    if effect == CombatStatEffectV1::GainPillzEqualToFinalDamageOnVictory {
        if source != CombatStatEffectSourceV1::Ability {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryPillzPerDamageSource,
            ));
        }
        if !matches!(
            predicate,
            CombatStatPredicateV1::Always | CombatStatPredicateV1::SelectedHandSlotsMatch
        ) {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryPillzPerDamagePredicate,
            ));
        }
        return Ok(());
    }
    if let CombatStatEffectV1::GainLifePerFinalDamageOnVictory {
        life_per_damage,
        maximum,
    } = effect
    {
        if source != CombatStatEffectSourceV1::Ability {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryLifePerDamageSource,
            ));
        }
        if life_per_damage == 0 {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryLifePerDamageMagnitude,
            ));
        }
        // A cap has only ever been printed without a previous-round prefix, so the two
        // must never arrive together on one plan.
        if !matches!(
            predicate,
            CombatStatPredicateV1::Always
                | CombatStatPredicateV1::OwnerLostPreviousRound
                | CombatStatPredicateV1::OwnerWonPreviousRound
        ) || (maximum > 0 && predicate != CombatStatPredicateV1::Always)
        {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryLifePerDamagePredicate,
            ));
        }
        return Ok(());
    }
    if let CombatStatEffectV1::GainLifeOnDefeat { life } = effect {
        if source != CombatStatEffectSourceV1::Ability {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::DefeatLifeSource,
            ));
        }
        if life == 0 {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::DefeatLifeMagnitude,
            ));
        }
        if predicate != CombatStatPredicateV1::Always {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::DefeatLifePredicate,
            ));
        }
        return Ok(());
    }
    if let CombatStatEffectV1::ReanimateLife { life } = effect {
        if source != CombatStatEffectSourceV1::Ability {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::ReanimateLifeSource,
            ));
        }
        if life == 0 {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::ReanimateLifeMagnitude,
            ));
        }
        if predicate != CombatStatPredicateV1::Always {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::ReanimateLifePredicate,
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

const fn reprisal_stop_opponent_ability_id_is_reserved(source_id: u32) -> bool {
    matches!(source_id, 1310 | 2073)
}

const fn komboka_victory_pillz_and_life_id_is_reserved(source_id: u32) -> bool {
    source_id == 1714
}

const KOMBOKA_EFFECTIVE_CLAN_ID: u32 = 54;

fn reprisal_stop_opponent_ability_card_matches(source_id: u32, key: CardKey) -> bool {
    matches!(
        (source_id, key),
        (1310, CardKey { id: 1498, level: 4 }) | (2073, CardKey { id: 2042, level: 3 })
    )
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
        // Copy is substituted for the adopted opposing plan before this point, so an
        // unresolved Copy contributes nothing rather than silently acting as itself.
        CombatStatSourcePlanV1::CopyOpponentSource { .. }
        | CombatStatSourcePlanV1::Absent
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
            selected[PlayerId::P2],
            PlayerId::P1,
            first_mover,
            validated[PlayerId::P1].slot,
            validated[PlayerId::P2].slot,
            previous_round_winner,
        ),
        resolution_card_plan(
            selected[PlayerId::P2],
            selected[PlayerId::P1],
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

/// Resolve one source against the opposing selected card. A Copy whose condition holds
/// adopts that card's corresponding immutable plan; everything else is already concrete.
/// The adopted plan is never itself a Copy, which construction has already guaranteed.
///
/// A Copy whose condition fails contributes nothing at all, which is not the same as
/// adopting an absent source: the distinction is invisible today because both produce no
/// effect, and it is kept explicit so a future Copy effect cannot quietly acquire one.
#[allow(clippy::too_many_arguments)]
fn resolved_source_plan(
    plan: CombatStatSourcePlanV1,
    opponent: CombatStatCardPlanV1,
    owner: PlayerId,
    first_mover: PlayerId,
    owner_slot: HandSlot,
    opponent_slot: HandSlot,
    previous_round_winner: Option<PlayerId>,
) -> CombatStatSourcePlanV1 {
    match plan {
        CombatStatSourcePlanV1::CopyOpponentSource {
            copied, predicate, ..
        } => {
            if !predicate_matches(
                predicate,
                owner,
                first_mover,
                owner_slot,
                opponent_slot,
                previous_round_winner,
            ) {
                return CombatStatSourcePlanV1::Absent;
            }
            match copied {
                CopiedSourceKindV1::Ability => opponent.ability,
                CopiedSourceKindV1::Bonus => opponent.bonus,
            }
        }
        other => other,
    }
}

fn resolution_card_plan(
    plan: CombatStatCardPlanV1,
    opponent_plan: CombatStatCardPlanV1,
    owner: PlayerId,
    first_mover: PlayerId,
    owner_slot: HandSlot,
    opponent_slot: HandSlot,
    previous_round_winner: Option<PlayerId>,
) -> ResolutionCardPlan {
    // Resolve each source once. Copy substitution and the predicate are the same work for
    // the combat effect and the post-round effect, and a source only ever supplies one of
    // the two, so doing it twice per source was only ever duplicated cost.
    let source = |plan: CombatStatSourcePlanV1, support_count: u16| {
        let effect = active_effect(
            resolved_source_plan(
                plan,
                opponent_plan,
                owner,
                first_mover,
                owner_slot,
                opponent_slot,
                previous_round_winner,
            ),
            owner,
            first_mover,
            owner_slot,
            opponent_slot,
            previous_round_winner,
        );
        ResolutionSourcePlan {
            effect: effect.and_then(shared_effect),
            post_round: effect.and_then(shared_post_round_effect),
            support_count,
        }
    };
    ResolutionCardPlan {
        ability: source(plan.ability, plan.source_ability_support_count),
        bonus: source(plan.bonus, plan.source_bonus_support_count),
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
                CombatStatMagnitudeV1::OpponentDamage => DiagnosticMagnitudeV1::OpponentDamage,
            },
        },
        CombatStatEffectV1::StopOpponentAbility => DiagnosticCombatEffectV1::StopOpponentAbility,
        CombatStatEffectV1::StopOpponentBonus => DiagnosticCombatEffectV1::StopOpponentBonus,
        CombatStatEffectV1::ProtectOwnCombatStat { stat } => {
            DiagnosticCombatEffectV1::ProtectOwnCombatStat {
                stat: match stat {
                    CombatStatAttributeV1::Attack => DiagnosticCombatStatV1::Attack,
                    CombatStatAttributeV1::Damage => DiagnosticCombatStatV1::Damage,
                    CombatStatAttributeV1::Power => DiagnosticCombatStatV1::Power,
                    CombatStatAttributeV1::PowerAndDamage => DiagnosticCombatStatV1::PowerAndDamage,
                },
            }
        }
        CombatStatEffectV1::ProtectOwnAbility => DiagnosticCombatEffectV1::ProtectOwnAbility,
        CombatStatEffectV1::ProtectOwnBonus => DiagnosticCombatEffectV1::ProtectOwnBonus,
        CombatStatEffectV1::CopyOpponentPrintedCombatStat { stat } => {
            DiagnosticCombatEffectV1::CopyOpponentPrintedCombatStat {
                stat: match stat {
                    CombatStatAttributeV1::Attack => DiagnosticCombatStatV1::Attack,
                    CombatStatAttributeV1::Damage => DiagnosticCombatStatV1::Damage,
                    CombatStatAttributeV1::Power => DiagnosticCombatStatV1::Power,
                    CombatStatAttributeV1::PowerAndDamage => DiagnosticCombatStatV1::PowerAndDamage,
                },
            }
        }
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
        | CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat
        | CombatStatEffectV1::GainOnePillzAndLifeOnVictory
        | CombatStatEffectV1::GainTwoPillzOnDefeatMaxEleven
        | CombatStatEffectV1::GainLifeEqualToFinalDamageOnCourageVictory
        | CombatStatEffectV1::ReduceOpponentLifeOnVictory { .. }
        | CombatStatEffectV1::GainLifeOnVictory { .. }
        | CombatStatEffectV1::GainPillzOnVictory { .. }
        | CombatStatEffectV1::ReduceOpponentPillzOnVictory { .. }
        | CombatStatEffectV1::ReduceOpponentPillzOnDefeat { .. }
        | CombatStatEffectV1::GainPillzEqualToFinalDamageOnVictory
        | CombatStatEffectV1::GainLifePerFinalDamageOnVictory { .. }
        | CombatStatEffectV1::GainLifeOnDefeat { .. }
        | CombatStatEffectV1::ReanimateLife { .. }
        | CombatStatEffectV1::GainLifeOnVictoryOrDefeat { .. }
        | CombatStatEffectV1::ReduceOpponentLifeOnVictoryOrDefeat { .. }
        | CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars { .. }
        | CombatStatEffectV1::ReduceOpponentLifeOnDefeat { .. }
        | CombatStatEffectV1::ReduceOpponentLifeOnKillshot { .. }
        | CombatStatEffectV1::ReduceBothPlayersLife { .. }
        | CombatStatEffectV1::HealLifeOnVictory { .. }
        | CombatStatEffectV1::RegenLifeOnVictory { .. }
        | CombatStatEffectV1::PoisonOpponentLifeOnVictory { .. }
        | CombatStatEffectV1::ToxinOpponentLifeOnVictory { .. } => return None,
    })
}

fn shared_post_round_effect(effect: CombatStatEffectV1) -> Option<PostRoundSourceEffect> {
    match effect {
        CombatStatEffectV1::RecoverPaidPillzOnDefeat => Some(PostRoundSourceEffect::Fixed(
            PostRoundEffect::RecoverPaidPillzOnDefeat,
        )),
        CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat => Some(PostRoundSourceEffect::Fixed(
            PostRoundEffect::GainOnePillzOnVictoryOrDefeat,
        )),
        CombatStatEffectV1::GainOnePillzAndLifeOnVictory => Some(PostRoundSourceEffect::Fixed(
            PostRoundEffect::GainOnePillzAndLifeOnVictory,
        )),
        CombatStatEffectV1::GainTwoPillzOnDefeatMaxEleven => Some(PostRoundSourceEffect::Fixed(
            PostRoundEffect::GainTwoPillzOnDefeatMaxEleven,
        )),
        CombatStatEffectV1::GainLifeEqualToFinalDamageOnCourageVictory => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::GainLifeEqualToFinalDamageOnCourageVictory,
            ))
        }
        CombatStatEffectV1::ReduceOpponentLifeOnVictory { life, minimum } => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::ReduceOpponentLifeOnVictory { life, minimum },
            ))
        }
        CombatStatEffectV1::GainLifeOnVictory { life } => Some(PostRoundSourceEffect::Fixed(
            PostRoundEffect::GainLifeOnVictory(life),
        )),
        CombatStatEffectV1::GainPillzOnVictory { pillz } => Some(PostRoundSourceEffect::Fixed(
            PostRoundEffect::GainPillzOnVictory(pillz),
        )),
        CombatStatEffectV1::ReduceOpponentPillzOnVictory { pillz, minimum } => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::ReduceOpponentPillzOnVictory { pillz, minimum },
            ))
        }
        CombatStatEffectV1::ReduceOpponentPillzOnDefeat { pillz, minimum } => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::ReduceOpponentPillzOnDefeat { pillz, minimum },
            ))
        }
        CombatStatEffectV1::GainPillzEqualToFinalDamageOnVictory => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::GainPillzEqualToFinalDamageOnVictory),
        ),
        CombatStatEffectV1::GainLifePerFinalDamageOnVictory {
            life_per_damage,
            maximum,
        } => Some(PostRoundSourceEffect::Fixed(
            PostRoundEffect::GainLifePerFinalDamageOnVictory {
                life_per_damage,
                maximum,
            },
        )),
        CombatStatEffectV1::GainLifeOnDefeat { life } => Some(PostRoundSourceEffect::Fixed(
            PostRoundEffect::GainLifeOnDefeat(life),
        )),
        CombatStatEffectV1::ReanimateLife { life } => Some(PostRoundSourceEffect::Fixed(
            PostRoundEffect::ReanimateLife(life),
        )),
        CombatStatEffectV1::GainLifeOnVictoryOrDefeat { life } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::GainLifeOnVictoryOrDefeat { life }),
        ),
        CombatStatEffectV1::ReduceOpponentLifeOnVictoryOrDefeat { life, minimum } => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::ReduceOpponentLifeOnVictoryOrDefeat { life, minimum },
            ))
        }
        CombatStatEffectV1::ReduceOpponentLifeOnDefeat { life, minimum } => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::ReduceOpponentLifeOnDefeat { life, minimum },
            ))
        }
        CombatStatEffectV1::ReduceOpponentLifeOnKillshot { life, minimum } => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::ReduceOpponentLifeOnKillshot { life, minimum },
            ))
        }
        CombatStatEffectV1::ReduceBothPlayersLife { life, minimum } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::ReduceBothPlayersLife { life, minimum }),
        ),
        CombatStatEffectV1::HealLifeOnVictory { life, maximum } => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::LatchOnVictory(LatchedEffectV1::HealLife { life, maximum }),
            ))
        }
        CombatStatEffectV1::RegenLifeOnVictory { life, maximum } => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::LatchOnVictory(LatchedEffectV1::RegenLife { life, maximum }),
            ))
        }
        CombatStatEffectV1::PoisonOpponentLifeOnVictory { life, minimum } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::LatchOnVictory(
                LatchedEffectV1::PoisonOpponentLife { life, minimum },
            )),
        ),
        CombatStatEffectV1::ToxinOpponentLifeOnVictory { life, minimum } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::LatchOnVictory(
                LatchedEffectV1::ToxinOpponentLife { life, minimum },
            )),
        ),
        CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars { per_star, minimum } => {
            Some(
                PostRoundSourceEffect::ReduceOpponentLifeOnVictoryPerOpponentStars {
                    per_star,
                    minimum,
                },
            )
        }
        CombatStatEffectV1::ModifyCombatStat { .. }
        | CombatStatEffectV1::StopOpponentAbility
        | CombatStatEffectV1::StopOpponentBonus
        | CombatStatEffectV1::CancelOpponentCombatStatModifiers { .. }
        | CombatStatEffectV1::ProtectOwnCombatStat { .. }
        | CombatStatEffectV1::ProtectOwnAbility
        | CombatStatEffectV1::ProtectOwnBonus
        | CombatStatEffectV1::CopyOpponentPrintedCombatStat { .. } => None,
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

    fn set_p1_card_key(spec: &mut CombatStatDiagnosticMatchSpecV1, key: CardKey) {
        spec.base_rules.players[PlayerId::P1].hand[0].key = key;
        spec.cards[PlayerId::P1][0].key = key;
    }

    fn set_p1_effective_clan_id(spec: &mut CombatStatDiagnosticMatchSpecV1, clan_id: u32) {
        spec.cards[PlayerId::P1][0].effective_clan_id = clan_id;
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
    fn reprisal_soa_is_live_only_when_its_owner_moves_second_and_undo_restores() {
        let mut spec = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1310,
            CombatStatEffectV1::StopOpponentAbility,
            0,
        );
        set_p1_card_key(&mut spec, CardKey::new(1498, 4));
        spec.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1310,
            predicate: CombatStatPredicateV1::OwnerMovesSecond,
            effect: CombatStatEffectV1::StopOpponentAbility,
        };
        spec.base_rules.players[PlayerId::P2].hand[0].power = 6;
        spec.cards[PlayerId::P2][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 2,
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::ModifyCombatStat {
                side: CombatStatAffectedSideV1::Player,
                stat: CombatStatAttributeV1::Power,
                operation: CombatStatOperationV1::Increase,
                value: 3,
                minimum: None,
                maximum: None,
                multiplier: CombatStatMagnitudeV1::Fixed,
            },
        };

        let mut first = CombatStatDiagnosticV1::new(spec.clone()).unwrap();
        let (report, _) = first.make(input(0, false)).unwrap();
        // P1 moved first, so Reprisal is inactive and P2's Ability is live.
        assert_eq!(report.cards[PlayerId::P2].power, 9);

        let mut second = CombatStatDiagnosticV1::new(spec).unwrap();
        let before = second.position().clone();
        let mut before_hasher = DefaultHasher::new();
        before.hash(&mut before_hasher);
        let before_hash = before_hasher.finish();
        let mut second_input = input(0, false);
        second_input.first_mover = PlayerId::P2;
        let (report, undo) = second.make(second_input).unwrap();
        // P1 now moved second, so its SOA is live before PRE4 and suppresses P2's Ability.
        assert_eq!(report.cards[PlayerId::P2].power, 6);
        second.unmake(undo);
        assert_eq!(second.position(), &before);
        let mut restored_hasher = DefaultHasher::new();
        second.position().hash(&mut restored_hasher);
        assert_eq!(restored_hasher.finish(), before_hash);
    }

    #[test]
    fn reprisal_soa_stops_defeat_life_as_in_capture_1069193_round_three() {
        // The full capture cannot yet replay because round zero selects Komboka's unsupported
        // +1 Pillz And Life. This isolated server-backed round keeps the observed arithmetic:
        // Spidee moves second, deals six to an opponent on 18, and their Defeat +2 Life is
        // stopped, leaving 12 rather than 14.
        let mut spec = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1310,
            CombatStatEffectV1::StopOpponentAbility,
            0,
        );
        set_p1_card_key(&mut spec, CardKey::new(1498, 4));
        spec.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1310,
            predicate: CombatStatPredicateV1::OwnerMovesSecond,
            effect: CombatStatEffectV1::StopOpponentAbility,
        };
        spec.base_rules.players[PlayerId::P1].hand[0].power = 40;
        spec.base_rules.players[PlayerId::P1].hand[0].damage = 6;
        spec.base_rules.players[PlayerId::P2].initial_life = 18;
        spec.base_rules.players[PlayerId::P2].hand[0].power = 6;
        spec.cards[PlayerId::P2][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 4635,
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::GainLifeOnDefeat { life: 2 },
        };
        let mut game = CombatStatDiagnosticV1::new(spec).unwrap();
        let before = game.position().clone();
        let mut before_hasher = DefaultHasher::new();
        before.hash(&mut before_hasher);
        let before_hash = before_hasher.finish();
        let mut round = input(0, false);
        round.first_mover = PlayerId::P2;
        let (report, undo) = game.make(round).unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P2].life, 12);
        game.unmake(undo);
        assert_eq!(game.position(), &before);
        let mut restored_hasher = DefaultHasher::new();
        game.position().hash(&mut restored_hasher);
        assert_eq!(restored_hasher.finish(), before_hash);
    }

    #[test]
    fn reprisal_soa_participates_in_the_existing_soa_cycle_without_leaking_state() {
        let mut spec = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1310,
            CombatStatEffectV1::StopOpponentAbility,
            0,
        );
        set_p1_card_key(&mut spec, CardKey::new(1498, 4));
        spec.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1310,
            predicate: CombatStatPredicateV1::OwnerMovesSecond,
            effect: CombatStatEffectV1::StopOpponentAbility,
        };
        spec.cards[PlayerId::P2][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 41,
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::StopOpponentAbility,
        };
        let mut game = CombatStatDiagnosticV1::new(spec).unwrap();
        let before = game.position().clone();
        let mut before_hasher = DefaultHasher::new();
        before.hash(&mut before_hasher);
        let before_hash = before_hasher.finish();
        let mut round = input(0, false);
        round.first_mover = PlayerId::P2;
        let (_, undo) = game.make(round).unwrap();
        game.unmake(undo);
        assert_eq!(game.position(), &before);
        let mut restored_hasher = DefaultHasher::new();
        game.position().hash(&mut restored_hasher);
        assert_eq!(restored_hasher.finish(), before_hash);
    }

    #[test]
    fn reprisal_soa_public_plans_are_ability_identity_and_predicate_locked() {
        for (id, key) in [(1310, CardKey::new(1498, 4)), (2073, CardKey::new(2042, 3))] {
            let mut valid = spec_with_p1(
                CombatStatEffectSourceV1::Ability,
                id,
                CombatStatEffectV1::StopOpponentAbility,
                0,
            );
            set_p1_card_key(&mut valid, key);
            valid.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
                source_id: id,
                predicate: CombatStatPredicateV1::OwnerMovesSecond,
                effect: CombatStatEffectV1::StopOpponentAbility,
            };
            assert!(CombatStatDiagnosticV1::new(valid).is_ok(), "id={id}");
        }

        let mut wrong_id = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1,
            CombatStatEffectV1::StopOpponentAbility,
            0,
        );
        wrong_id.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1,
            predicate: CombatStatPredicateV1::OwnerMovesSecond,
            effect: CombatStatEffectV1::StopOpponentAbility,
        };
        assert!(matches!(
            CombatStatDiagnosticV1::new(wrong_id),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::ReprisalStopOpponentAbilityIdentity,
                ..
            })
        ));

        let mut wrong_source = spec_with_p1(
            CombatStatEffectSourceV1::Bonus,
            1310,
            CombatStatEffectV1::StopOpponentAbility,
            0,
        );
        wrong_source.cards[PlayerId::P1][0].bonus = CombatStatSourcePlanV1::Execute {
            source_id: 1310,
            predicate: CombatStatPredicateV1::OwnerMovesSecond,
            effect: CombatStatEffectV1::StopOpponentAbility,
        };
        assert!(matches!(
            CombatStatDiagnosticV1::new(wrong_source),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::ReprisalStopOpponentAbilityIdentity,
                ..
            })
        ));

        for effect in [
            CombatStatEffectV1::StopOpponentAbility,
            CombatStatEffectV1::ModifyCombatStat {
                side: CombatStatAffectedSideV1::Player,
                stat: CombatStatAttributeV1::Power,
                operation: CombatStatOperationV1::Increase,
                value: 1,
                minimum: None,
                maximum: None,
                multiplier: CombatStatMagnitudeV1::Fixed,
            },
        ] {
            // Default predicate is Always: neither generic unconditional SOA nor a numeric
            // plan may borrow a reserved Reprisal identity through the Bonus source.
            let wrong_source_always =
                spec_with_p1(CombatStatEffectSourceV1::Bonus, 1310, effect, 0);
            assert!(matches!(
                CombatStatDiagnosticV1::new(wrong_source_always),
                Err(CombatStatPlanErrorV1::InvalidExecute {
                    reason: InvalidCombatStatPlanReasonV1::ReprisalStopOpponentAbilityIdentity,
                    ..
                })
            ));
        }

        let mut wrong_predicate = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1310,
            CombatStatEffectV1::StopOpponentAbility,
            0,
        );
        set_p1_card_key(&mut wrong_predicate, CardKey::new(1498, 4));
        wrong_predicate.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1310,
            predicate: CombatStatPredicateV1::OwnerMovesFirst,
            effect: CombatStatEffectV1::StopOpponentAbility,
        };
        assert!(matches!(
            CombatStatDiagnosticV1::new(wrong_predicate),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::ReprisalStopOpponentAbilityPredicate,
                ..
            })
        ));

        let mut unconditional_reprisal = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1310,
            CombatStatEffectV1::StopOpponentAbility,
            0,
        );
        set_p1_card_key(&mut unconditional_reprisal, CardKey::new(1498, 4));
        assert!(matches!(
            CombatStatDiagnosticV1::new(unconditional_reprisal),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::ReprisalStopOpponentAbilityPredicate,
                ..
            })
        ));

        let mut wrong_effect = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1310,
            CombatStatEffectV1::StopOpponentBonus,
            0,
        );
        set_p1_card_key(&mut wrong_effect, CardKey::new(1498, 4));
        wrong_effect.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1310,
            predicate: CombatStatPredicateV1::OwnerMovesSecond,
            effect: CombatStatEffectV1::StopOpponentBonus,
        };
        assert!(matches!(
            CombatStatDiagnosticV1::new(wrong_effect),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::ReprisalStopOpponentAbilityEffect,
                ..
            })
        ));

        for (source_id, wrong_key) in [
            (1310, CardKey::new(1498, 3)),
            (1310, CardKey::new(2042, 3)),
            (2073, CardKey::new(2042, 2)),
            (2073, CardKey::new(1498, 4)),
        ] {
            let mut wrong_card = spec_with_p1(
                CombatStatEffectSourceV1::Ability,
                source_id,
                CombatStatEffectV1::StopOpponentAbility,
                0,
            );
            set_p1_card_key(&mut wrong_card, wrong_key);
            wrong_card.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
                source_id,
                predicate: CombatStatPredicateV1::OwnerMovesSecond,
                effect: CombatStatEffectV1::StopOpponentAbility,
            };
            assert!(matches!(
                CombatStatDiagnosticV1::new(wrong_card),
                Err(CombatStatPlanErrorV1::InvalidExecute {
                    reason: InvalidCombatStatPlanReasonV1::ReprisalStopOpponentAbilityCard,
                    ..
                })
            ));
        }

        // Existing unconditional public plans remain intentionally generic.
        assert!(CombatStatDiagnosticV1::new(spec_with_p1(
            CombatStatEffectSourceV1::Bonus,
            1,
            CombatStatEffectV1::StopOpponentAbility,
            0,
        ))
        .is_ok());
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

    #[test]
    fn komboka_composite_bonus_is_exact_winner_only_ko_safe_and_undo_atomic() {
        let komboka = CombatStatEffectV1::GainOnePillzAndLifeOnVictory;
        let mut winner_spec = spec_with_p1(CombatStatEffectSourceV1::Bonus, 1714, komboka, 3);
        set_p1_effective_clan_id(&mut winner_spec, KOMBOKA_EFFECTIVE_CLAN_ID);
        winner_spec.base_rules.players[PlayerId::P1].hand[0].power = 40;
        // A winning Komboka bonus still pays after it KOs the opposing player.
        winner_spec.base_rules.players[PlayerId::P2].initial_life = 3;
        let mut winner = CombatStatDiagnosticV1::new(winner_spec).unwrap();
        let before = winner.position().clone();
        let mut before_hasher = DefaultHasher::new();
        before.hash(&mut before_hasher);
        let before_hash = before_hasher.finish();
        let (report, undo) = winner.make(input(3, false)).unwrap();
        assert_eq!(report.status, MatchStatus::Won(PlayerId::P1));
        assert_eq!(report.players[PlayerId::P1].pillz, 1);
        assert_eq!(report.players[PlayerId::P1].life, 21);
        assert_eq!(report.players[PlayerId::P2].life, 0);
        winner.unmake(undo);
        assert_eq!(winner.position(), &before);
        let mut restored_hasher = DefaultHasher::new();
        winner.position().hash(&mut restored_hasher);
        assert_eq!(restored_hasher.finish(), before_hash);

        let mut losing_spec = spec_with_p1(CombatStatEffectSourceV1::Bonus, 1714, komboka, 3);
        set_p1_effective_clan_id(&mut losing_spec, KOMBOKA_EFFECTIVE_CLAN_ID);
        let mut losing = CombatStatDiagnosticV1::new(losing_spec).unwrap();
        let (report, _) = losing.make(input(0, false)).unwrap();
        assert!(!report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P1].life, 17);
        assert_eq!(report.players[PlayerId::P1].pillz, 3);

        let mut losing_ko_spec = spec_with_p1(CombatStatEffectSourceV1::Bonus, 1714, komboka, 0);
        set_p1_effective_clan_id(&mut losing_ko_spec, KOMBOKA_EFFECTIVE_CLAN_ID);
        losing_ko_spec.base_rules.players[PlayerId::P1].initial_life = 2;
        let mut losing_ko = CombatStatDiagnosticV1::new(losing_ko_spec).unwrap();
        let (report, _) = losing_ko.make(input(0, false)).unwrap();
        assert_eq!(report.status, MatchStatus::Won(PlayerId::P2));
        assert_eq!(report.players[PlayerId::P1].life, 0);
        assert_eq!(report.players[PlayerId::P1].pillz, 0);

        let mut pillz_overflow_spec =
            spec_with_p1(CombatStatEffectSourceV1::Bonus, 1714, komboka, u16::MAX);
        set_p1_effective_clan_id(&mut pillz_overflow_spec, KOMBOKA_EFFECTIVE_CLAN_ID);
        pillz_overflow_spec.base_rules.players[PlayerId::P1].hand[0].power = 40;
        let mut pillz_overflow = CombatStatDiagnosticV1::new(pillz_overflow_spec).unwrap();
        let before = pillz_overflow.position().clone();
        assert!(matches!(
            pillz_overflow.make(input(0, false)),
            Err(CombatStatDiagnosticErrorV1::BaseRules(
                BaseRulesError::PillzIncreaseOverflow {
                    player: PlayerId::P1
                }
            ))
        ));
        assert_eq!(pillz_overflow.position(), &before);

        let mut life_overflow_spec =
            spec_with_p1(CombatStatEffectSourceV1::Bonus, 1714, komboka, 0);
        set_p1_effective_clan_id(&mut life_overflow_spec, KOMBOKA_EFFECTIVE_CLAN_ID);
        life_overflow_spec.base_rules.players[PlayerId::P1].initial_life = u16::MAX;
        life_overflow_spec.base_rules.players[PlayerId::P1].hand[0].power = 40;
        let mut life_overflow = CombatStatDiagnosticV1::new(life_overflow_spec).unwrap();
        let before = life_overflow.position().clone();
        assert!(matches!(
            life_overflow.make(input(0, false)),
            Err(CombatStatDiagnosticErrorV1::BaseRules(
                BaseRulesError::LifeIncreaseOverflow {
                    player: PlayerId::P1
                }
            ))
        ));
        // Pillz was checked and tentatively added before Life, but neither part can leak
        // from the replacement position when the second checked addition overflows.
        assert_eq!(life_overflow.position(), &before);
    }

    #[test]
    fn komboka_public_plan_reserves_1714_for_its_exact_bonus_effect_and_predicate() {
        let komboka = CombatStatEffectV1::GainOnePillzAndLifeOnVictory;
        let mut valid = spec_with_p1(CombatStatEffectSourceV1::Bonus, 1714, komboka, 3);
        set_p1_effective_clan_id(&mut valid, KOMBOKA_EFFECTIVE_CLAN_ID);
        assert!(CombatStatDiagnosticV1::new(valid).is_ok());

        assert!(matches!(
            CombatStatDiagnosticV1::new(spec_with_p1(
                CombatStatEffectSourceV1::Bonus,
                1714,
                komboka,
                3,
            )),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::KombokaVictoryPillzAndLifeClan,
                ..
            })
        ));

        for effect in [
            CombatStatEffectV1::GainLifeOnVictory { life: 1 },
            CombatStatEffectV1::ModifyCombatStat {
                side: CombatStatAffectedSideV1::Player,
                stat: CombatStatAttributeV1::Power,
                operation: CombatStatOperationV1::Increase,
                value: 1,
                minimum: None,
                maximum: None,
                multiplier: CombatStatMagnitudeV1::Fixed,
            },
        ] {
            assert!(matches!(
                CombatStatDiagnosticV1::new(spec_with_p1(
                    CombatStatEffectSourceV1::Bonus,
                    1714,
                    effect,
                    3,
                )),
                Err(CombatStatPlanErrorV1::InvalidExecute {
                    reason: InvalidCombatStatPlanReasonV1::KombokaVictoryPillzAndLifeEffect,
                    ..
                })
            ));
        }
        assert!(matches!(
            CombatStatDiagnosticV1::new(spec_with_p1(
                CombatStatEffectSourceV1::Ability,
                1714,
                komboka,
                3,
            )),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::KombokaVictoryPillzAndLifeIdentity,
                ..
            })
        ));
        let mut wrong_predicate = spec_with_p1(CombatStatEffectSourceV1::Bonus, 1714, komboka, 3);
        set_p1_effective_clan_id(&mut wrong_predicate, KOMBOKA_EFFECTIVE_CLAN_ID);
        wrong_predicate.cards[PlayerId::P1][0].bonus = CombatStatSourcePlanV1::Execute {
            source_id: 1714,
            predicate: CombatStatPredicateV1::OwnerWonPreviousRound,
            effect: komboka,
        };
        assert!(matches!(
            CombatStatDiagnosticV1::new(wrong_predicate),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::KombokaVictoryPillzAndLifePredicate,
                ..
            })
        ));
        assert!(matches!(
            CombatStatDiagnosticV1::new(spec_with_p1(
                CombatStatEffectSourceV1::Bonus,
                1,
                komboka,
                3,
            )),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::KombokaVictoryPillzAndLifeIdentity,
                ..
            })
        ));
    }

    #[test]
    fn komboka_bonus_obeys_stop_bonus_but_not_stop_opponent_ability() {
        let komboka = CombatStatEffectV1::GainOnePillzAndLifeOnVictory;
        let mut stopped_spec = spec_with_p1(CombatStatEffectSourceV1::Bonus, 1714, komboka, 3);
        set_p1_effective_clan_id(&mut stopped_spec, KOMBOKA_EFFECTIVE_CLAN_ID);
        stopped_spec.base_rules.players[PlayerId::P1].hand[0].power = 40;
        stopped_spec.cards[PlayerId::P2][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1,
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::StopOpponentBonus,
        };
        let mut stopped = CombatStatDiagnosticV1::new(stopped_spec).unwrap();
        let before = stopped.position().clone();
        let mut before_hasher = DefaultHasher::new();
        before.hash(&mut before_hasher);
        let before_hash = before_hasher.finish();
        let (report, undo) = stopped.make(input(3, false)).unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P1].pillz, 0);
        assert_eq!(report.players[PlayerId::P1].life, 20);
        stopped.unmake(undo);
        assert_eq!(stopped.position(), &before);
        let mut restored_hasher = DefaultHasher::new();
        stopped.position().hash(&mut restored_hasher);
        assert_eq!(restored_hasher.finish(), before_hash);

        let mut soa_spec = spec_with_p1(CombatStatEffectSourceV1::Bonus, 1714, komboka, 3);
        set_p1_effective_clan_id(&mut soa_spec, KOMBOKA_EFFECTIVE_CLAN_ID);
        soa_spec.base_rules.players[PlayerId::P1].hand[0].power = 40;
        soa_spec.cards[PlayerId::P2][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1,
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::StopOpponentAbility,
        };
        let mut soa = CombatStatDiagnosticV1::new(soa_spec).unwrap();
        let (report, _) = soa.make(input(3, false)).unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P1].pillz, 1);
        assert_eq!(report.players[PlayerId::P1].life, 21);
    }

    #[test]
    fn victory_life_direct_plan_is_positive_winner_only_bonus_before_ability_and_atomic() {
        let bonus = CombatStatEffectV1::GainLifeOnVictory { life: 2 };
        let ability = CombatStatEffectV1::GainLifeOnVictory { life: 1 };
        let mut ordered = spec_with_p1(CombatStatEffectSourceV1::Bonus, 888, bonus, 3);
        ordered.base_rules.players[PlayerId::P1].hand[0].power = 40;
        ordered.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 889,
            predicate: CombatStatPredicateV1::Always,
            effect: ability,
        };
        let mut ordered = CombatStatDiagnosticV1::new(ordered).unwrap();
        let before = ordered.position().clone();
        let mut hasher = DefaultHasher::new();
        before.hash(&mut hasher);
        let before_hash = hasher.finish();
        let (report, undo) = ordered.make(input(3, false)).unwrap();
        assert!(report.cards[PlayerId::P1].won);
        // The Bonus registration is before Ability in the common END plan; both values
        // are carried by the plan rather than read from a registry on the hot path.
        assert_eq!(report.players[PlayerId::P1].life, 23);
        ordered.unmake(undo);
        assert_eq!(ordered.position(), &before);
        let mut restored = DefaultHasher::new();
        ordered.position().hash(&mut restored);
        assert_eq!(restored.finish(), before_hash);

        let mut zero = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            888,
            CombatStatEffectV1::GainLifeOnVictory { life: 0 },
            3,
        );
        zero.base_rules.players[PlayerId::P1].hand[0].power = 40;
        assert!(matches!(
            CombatStatDiagnosticV1::new(zero),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::VictoryLifeMagnitude,
                ..
            })
        ));

        // The two reviewed prefixed forms are the only predicates the grammar carries, and
        // only from the Ability slot: `Confidence :` and `Asymmetry:` are abilities, no
        // clan bonus prints either, and every other predicate is still out of the slice.
        for predicate in [
            CombatStatPredicateV1::OwnerWonPreviousRound,
            CombatStatPredicateV1::SelectedHandSlotsDiffer,
        ] {
            let mut conditional = spec_with_p1(CombatStatEffectSourceV1::Ability, 888, bonus, 3);
            conditional.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
                source_id: 888,
                predicate,
                effect: bonus,
            };
            assert!(CombatStatDiagnosticV1::new(conditional).is_ok());

            let mut from_bonus = spec_with_p1(CombatStatEffectSourceV1::Ability, 888, bonus, 3);
            from_bonus.cards[PlayerId::P1][0].bonus = CombatStatSourcePlanV1::Execute {
                source_id: 888,
                predicate,
                effect: bonus,
            };
            assert!(matches!(
                CombatStatDiagnosticV1::new(from_bonus),
                Err(CombatStatPlanErrorV1::InvalidExecute {
                    reason: InvalidCombatStatPlanReasonV1::VictoryLifePredicate,
                    ..
                })
            ));
        }

        let mut conditional = spec_with_p1(CombatStatEffectSourceV1::Ability, 888, bonus, 3);
        conditional.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 888,
            predicate: CombatStatPredicateV1::OwnerLostPreviousRound,
            effect: bonus,
        };
        assert!(matches!(
            CombatStatDiagnosticV1::new(conditional),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::VictoryLifePredicate,
                ..
            })
        ));

        let mut overflow = spec_with_p1(CombatStatEffectSourceV1::Ability, 888, bonus, 0);
        overflow.base_rules.players[PlayerId::P1].initial_life = u16::MAX;
        overflow.base_rules.players[PlayerId::P1].hand[0].power = 40;
        let mut overflow = CombatStatDiagnosticV1::new(overflow).unwrap();
        let before = overflow.position().clone();
        assert!(matches!(
            overflow.make(input(0, false)),
            Err(CombatStatDiagnosticErrorV1::BaseRules(
                BaseRulesError::LifeIncreaseOverflow {
                    player: PlayerId::P1
                }
            ))
        ));
        assert_eq!(overflow.position(), &before);
    }

    #[test]
    fn defeat_life_is_surviving_loss_only_and_reanimate_revives_before_status() {
        let defeat = CombatStatEffectV1::GainLifeOnDefeat { life: 2 };
        let reanimate = CombatStatEffectV1::ReanimateLife { life: 2 };

        let mut ordinary = spec_with_p1(CombatStatEffectSourceV1::Ability, 862, defeat, 0);
        ordinary.base_rules.players[PlayerId::P1].initial_life = 7;
        let mut ordinary = CombatStatDiagnosticV1::new(ordinary).unwrap();
        let (report, _) = ordinary.make(input(0, false)).unwrap();
        assert!(!report.cards[PlayerId::P1].won);
        // 7 - 3 damage + 2 Defeat Life.
        assert_eq!(report.players[PlayerId::P1].life, 6);

        let mut ordinary_ko = spec_with_p1(CombatStatEffectSourceV1::Ability, 862, defeat, 0);
        ordinary_ko.base_rules.players[PlayerId::P1].initial_life = 2;
        let mut ordinary_ko = CombatStatDiagnosticV1::new(ordinary_ko).unwrap();
        let (report, _) = ordinary_ko.make(input(0, false)).unwrap();
        assert_eq!(report.players[PlayerId::P1].life, 0);
        assert_eq!(report.status, MatchStatus::Won(PlayerId::P2));

        let mut winning = spec_with_p1(CombatStatEffectSourceV1::Ability, 862, defeat, 0);
        winning.base_rules.players[PlayerId::P1].initial_life = 7;
        winning.base_rules.players[PlayerId::P1].hand[0].power = 40;
        let mut winning = CombatStatDiagnosticV1::new(winning).unwrap();
        let (report, _) = winning.make(input(0, false)).unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P1].life, 7);

        let mut revive = spec_with_p1(CombatStatEffectSourceV1::Ability, 4951, reanimate, 0);
        revive.base_rules.players[PlayerId::P1].initial_life = 2;
        let mut revive = CombatStatDiagnosticV1::new(revive).unwrap();
        let before = revive.position().clone();
        let mut before_hasher = DefaultHasher::new();
        before.hash(&mut before_hasher);
        let before_hash = before_hasher.finish();
        let (report, undo) = revive.make(input(0, false)).unwrap();
        assert!(!report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P1].life, 2);
        assert_eq!(report.status, MatchStatus::Playing);
        revive.unmake(undo);
        assert_eq!(revive.position(), &before);
        let mut restored = DefaultHasher::new();
        revive.position().hash(&mut restored);
        assert_eq!(restored.finish(), before_hash);
    }

    #[test]
    fn reanimate_is_stopped_and_life_overflow_is_atomic() {
        let reanimate = CombatStatEffectV1::ReanimateLife { life: 2 };
        let mut stopped = spec_with_p1(CombatStatEffectSourceV1::Ability, 4951, reanimate, 0);
        stopped.base_rules.players[PlayerId::P1].initial_life = 7;
        stopped.cards[PlayerId::P2][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1,
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::StopOpponentAbility,
        };
        let mut stopped = CombatStatDiagnosticV1::new(stopped).unwrap();
        let (report, _) = stopped.make(input(0, false)).unwrap();
        assert_eq!(report.players[PlayerId::P1].life, 4);

        let mut overflow = spec_with_p1(CombatStatEffectSourceV1::Ability, 4951, reanimate, 0);
        overflow.base_rules.players[PlayerId::P1].initial_life = u16::MAX;
        // Keep the loss while making its damage zero, so Reanimate's checked add is the
        // operation which overflows and the complete replacement position must roll back.
        overflow.base_rules.players[PlayerId::P2].hand[0].damage = 0;
        let mut overflow = CombatStatDiagnosticV1::new(overflow).unwrap();
        let before = overflow.position().clone();
        assert!(matches!(
            overflow.make(input(0, false)),
            Err(CombatStatDiagnosticErrorV1::BaseRules(
                BaseRulesError::LifeIncreaseOverflow {
                    player: PlayerId::P1
                }
            ))
        ));
        assert_eq!(overflow.position(), &before);
    }

    #[test]
    fn defeat_life_and_reanimate_public_plans_are_ability_only_positive_and_unconditional() {
        for (effect, source_reason, magnitude_reason, predicate_reason) in [
            (
                CombatStatEffectV1::GainLifeOnDefeat { life: 2 },
                InvalidCombatStatPlanReasonV1::DefeatLifeSource,
                InvalidCombatStatPlanReasonV1::DefeatLifeMagnitude,
                InvalidCombatStatPlanReasonV1::DefeatLifePredicate,
            ),
            (
                CombatStatEffectV1::ReanimateLife { life: 2 },
                InvalidCombatStatPlanReasonV1::ReanimateLifeSource,
                InvalidCombatStatPlanReasonV1::ReanimateLifeMagnitude,
                InvalidCombatStatPlanReasonV1::ReanimateLifePredicate,
            ),
        ] {
            assert!(CombatStatDiagnosticV1::new(spec_with_p1(
                CombatStatEffectSourceV1::Ability,
                1,
                effect,
                0,
            ))
            .is_ok());
            assert!(matches!(
                CombatStatDiagnosticV1::new(spec_with_p1(
                    CombatStatEffectSourceV1::Bonus,
                    1,
                    effect,
                    0,
                )),
                Err(CombatStatPlanErrorV1::InvalidExecute { reason, .. }) if reason == source_reason
            ));
            let zero = match effect {
                CombatStatEffectV1::GainLifeOnDefeat { .. } => {
                    CombatStatEffectV1::GainLifeOnDefeat { life: 0 }
                }
                CombatStatEffectV1::ReanimateLife { .. } => {
                    CombatStatEffectV1::ReanimateLife { life: 0 }
                }
                _ => unreachable!(),
            };
            assert!(matches!(
                CombatStatDiagnosticV1::new(spec_with_p1(
                    CombatStatEffectSourceV1::Ability,
                    1,
                    zero,
                    0,
                )),
                Err(CombatStatPlanErrorV1::InvalidExecute { reason, .. }) if reason == magnitude_reason
            ));
            let mut conditional = spec_with_p1(CombatStatEffectSourceV1::Ability, 1, effect, 0);
            conditional.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
                source_id: 1,
                predicate: CombatStatPredicateV1::OwnerLostPreviousRound,
                effect,
            };
            assert!(matches!(
                CombatStatDiagnosticV1::new(conditional),
                Err(CombatStatPlanErrorV1::InvalidExecute { reason, .. }) if reason == predicate_reason
            ));
        }
    }

    #[test]
    fn argos_defeat_pillz_is_capped_after_bonus_and_skips_wins_or_kos() {
        let argos = CombatStatEffectV1::GainTwoPillzOnDefeatMaxEleven;
        let vod = CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat;
        let mut ordered = spec_with_p1(CombatStatEffectSourceV1::Ability, 1158, argos, 12);
        ordered.cards[PlayerId::P1][0].bonus = CombatStatSourcePlanV1::Execute {
            source_id: 1034,
            predicate: CombatStatPredicateV1::Always,
            effect: vod,
        };
        ordered.cards[PlayerId::P1][0].source_bonus_support_count = 1;
        let mut ordered = CombatStatDiagnosticV1::new(ordered).unwrap();
        let before = ordered.position().clone();
        let mut before_hasher = DefaultHasher::new();
        before.hash(&mut before_hasher);
        let before_hash = before_hasher.finish();
        let (report, undo) = ordered.make(input(2, false)).unwrap();
        assert!(!report.cards[PlayerId::P1].won);
        // 12 - 2 = 10; Bonus:1034 runs first to 11, then Argos is already capped.
        assert_eq!(report.players[PlayerId::P1].pillz, 11);
        ordered.unmake(undo);
        assert_eq!(ordered.position(), &before);
        let mut restored_hasher = DefaultHasher::new();
        ordered.position().hash(&mut restored_hasher);
        assert_eq!(restored_hasher.finish(), before_hash);

        let mut above = spec_with_p1(CombatStatEffectSourceV1::Ability, 1158, argos, 13);
        above.cards[PlayerId::P1][0].bonus = CombatStatSourcePlanV1::Execute {
            source_id: 1034,
            predicate: CombatStatPredicateV1::Always,
            effect: vod,
        };
        above.cards[PlayerId::P1][0].source_bonus_support_count = 1;
        let mut above = CombatStatDiagnosticV1::new(above).unwrap();
        let (report, _) = above.make(input(2, false)).unwrap();
        // 13 - 2 = 11; bonus first gives 12 and Argos must not lower it.
        assert_eq!(report.players[PlayerId::P1].pillz, 12);

        let mut winning = spec_with_p1(CombatStatEffectSourceV1::Ability, 1158, argos, 12);
        winning.base_rules.players[PlayerId::P1].hand[0].power = 40;
        let mut winning = CombatStatDiagnosticV1::new(winning).unwrap();
        let (report, _) = winning.make(input(2, false)).unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P1].pillz, 10);

        let mut zero = CombatStatDiagnosticV1::new(spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1158,
            argos,
            3,
        ))
        .unwrap();
        let (report, _) = zero.make(input(3, false)).unwrap();
        assert!(!report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P1].pillz, 2);

        let mut stopped_bonus = spec_with_p1(CombatStatEffectSourceV1::Ability, 1158, argos, 9);
        stopped_bonus.cards[PlayerId::P1][0].bonus = CombatStatSourcePlanV1::Execute {
            source_id: 1034,
            predicate: CombatStatPredicateV1::Always,
            effect: vod,
        };
        stopped_bonus.cards[PlayerId::P1][0].source_bonus_support_count = 1;
        stopped_bonus.cards[PlayerId::P2][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1,
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::StopOpponentBonus,
        };
        let mut stopped_bonus = CombatStatDiagnosticV1::new(stopped_bonus).unwrap();
        let (report, _) = stopped_bonus.make(input(2, false)).unwrap();
        // 9 - 2 = 7; Stop Bonus suppresses only VOD, so Argos still returns two.
        assert_eq!(report.players[PlayerId::P1].pillz, 9);

        let mut ko = spec_with_p1(CombatStatEffectSourceV1::Ability, 1158, argos, 12);
        ko.base_rules.players[PlayerId::P1].initial_life = 2;
        ko.cards[PlayerId::P1][0].bonus = CombatStatSourcePlanV1::Execute {
            source_id: 1034,
            predicate: CombatStatPredicateV1::Always,
            effect: vod,
        };
        ko.cards[PlayerId::P1][0].source_bonus_support_count = 1;
        let mut ko = CombatStatDiagnosticV1::new(ko).unwrap();
        let (report, _) = ko.make(input(2, false)).unwrap();
        assert_eq!(report.status, MatchStatus::Won(PlayerId::P2));
        assert_eq!(report.players[PlayerId::P1].life, 0);
        // VOD is the audited post-KO exception; Argos' ordinary Pillz gain is suppressed.
        assert_eq!(report.players[PlayerId::P1].pillz, 11);
    }

    #[test]
    fn argos_public_plans_are_identity_and_predicate_locked() {
        let effect = CombatStatEffectV1::GainTwoPillzOnDefeatMaxEleven;
        assert!(CombatStatDiagnosticV1::new(spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1158,
            effect,
            12,
        ))
        .is_ok());
        assert!(matches!(
            CombatStatDiagnosticV1::new(spec_with_p1(
                CombatStatEffectSourceV1::Bonus,
                1158,
                effect,
                12,
            )),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::ArgosDefeatCappedPillzIdentity,
                ..
            })
        ));
        let mut wrong_predicate = spec_with_p1(CombatStatEffectSourceV1::Ability, 1158, effect, 12);
        wrong_predicate.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1158,
            predicate: CombatStatPredicateV1::OwnerLostPreviousRound,
            effect,
        };
        assert!(matches!(
            CombatStatDiagnosticV1::new(wrong_predicate),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::ArgosDefeatCappedPillzPredicate,
                ..
            })
        ));
    }
}
