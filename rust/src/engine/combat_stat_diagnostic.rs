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
    conditional_stop_predicate_admitted, equalizer_opponent_life_on_victory_identity_matches,
    komboka_victory_pillz_and_life_identity_matches, opponent_can_stop_an_ability,
    permanent_predicate_admitted, victory_opponent_life_identity_matches,
    victory_opponent_life_predicate, victory_or_defeat_pillz_identity_matches,
};
use super::{
    BaseRulesError, BaseRulesGame, BaseRulesMatchSpec, BaseRulesPosition, BaseRulesRoundInput,
    BaseRulesRoundReport, BaseRulesUndo, ByPlayer, DiagnosticAffectedSideV1,
    DiagnosticCombatEffectV1, DiagnosticCombatStatV1, DiagnosticMagnitudeV1,
    DiagnosticStatOperationV1, HandSlot, LatchedEffectV1, LifeBeneficiaryV1, LifeWritesV1,
    PillzWritesV1, PlayerId, PostRoundEffect, PostRoundResourceV1, PostRoundSourceEffect,
    ValidatedSelection, WriteOutcomesV1, HAND_SIZE,
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
    /// `Cards`: the change lands on both selected cards, each clamped on its own. The
    /// owner's card takes it with the owner's own modifiers and the opposing card with the
    /// owner's opposing ones. Fixed magnitude, card abilities only.
    Both,
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
    /// Scaled by the opposing selected card's resolved Power, before any `Tune Out` reset.
    OpponentPower,
    /// `Brawl:`. Scaled by the number of distinct characters in the opposing hand sharing
    /// the opposing selected card's effective clan - the mirror of Support, which counts
    /// the owner's own hand.
    AntiSupport,
    /// `Per Life Left`. Scaled by the owner's own Life at the start of the round.
    OwnerLife,
    /// `Per Pillz Left`. The owner's Pillz at the start of the round, before the bet.
    OwnerPillz,
    /// `Per Pillz Lost`. The owner's match-start Pillz less their round-start Pillz.
    OwnerPillzLost,
    /// `/ Life Lost`. The owner's match-start Life less their round-start Life.
    OwnerLifeLost,
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
    /// `Night:` - Clint City is at night for this match. A match constant, so the source is
    /// present in every round and fires in all of them or in none.
    MatchIsNight,
    /// `Day:` - Clint City is in daylight for this match.
    MatchIsDay,
    /// `Unison :` - every card in the owner's hand shares the owner's selected card's
    /// effective clan (Oculus infiltration counts).
    OwnerHandUnison,
    /// `Stop:` - the owner's own ability was stopped by the opposing character. No captured
    /// round has ever shown it fire, so the projection models only the half the server has
    /// pinned: construction refuses any match in which an opposing source could stop the
    /// owner's ability, and within every match it admits the condition cannot hold.
    OwnerAbilityStopped,
    /// `[clan:A][clan:B] X`: the owner's selected card's effective clan is listed.
    OwnerClanIn(ClanSetV1),
    /// `After [clan:A] : X`: the card the owner played in the previous round has a listed
    /// canonical clan. Never holds in round 0.
    OwnerPreviousCardClanIn(ClanSetV1),
    /// `Versus [clan:A] : X`: some card in the opposing hand has a listed canonical clan.
    OpponentHandHasClan(ClanSetV1),
    /// `Bet > N Pillz: X`: the owner's Pillz for the round exceed N, counted as the server's
    /// `pillzUsed` - the free pill included, Fury's three excluded.
    OwnerPillzUsedAbove(u8),
    /// `Bet < N Pillz: X`: the owner's `pillzUsed` for the round is below N.
    OwnerPillzUsedBelow(u8),
    /// `Night: Confid.: X`: the match is at night and the owner won the previous round -
    /// the conjunction of `MatchIsNight` and `OwnerWonPreviousRound`, both already resolved
    /// before a round is prepared. Never holds in round 0 or by day.
    OwnerWonPreviousRoundAtNight,
    /// `[clan:A][clan:B] <prefix>: X`: the owner-clan gate of `OwnerClanIn` and one more
    /// condition the projection already resolves before a round is prepared - `Courage:`,
    /// `Repris.:` or `Asymm.:`/`Asy. :`. Both must hold. Card abilities only, over a fixed
    /// magnitude, a conditional Stop, a Copy or a latch (revision 70).
    OwnerClanInAnd(ClanSetV1, ClanConjunctV1),
}

/// The second condition of `OwnerClanInAnd`. Each is one of the plain predicates, so the
/// compound is exactly their conjunction and adds no new context.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ClanConjunctV1 {
    /// `Courage:` - the owner moves first.
    OwnerMovesFirst,
    /// `Repris.:` - the owner moves second.
    OwnerMovesSecond,
    /// `Asymm.:`/`Asy. :` - the two selected cards sit in different hand slots.
    SelectedHandSlotsDiffer,
}

impl ClanConjunctV1 {
    /// The plain predicate this conjunct is.
    pub const fn predicate(self) -> CombatStatPredicateV1 {
        match self {
            Self::OwnerMovesFirst => CombatStatPredicateV1::OwnerMovesFirst,
            Self::OwnerMovesSecond => CombatStatPredicateV1::OwnerMovesSecond,
            Self::SelectedHandSlotsDiffer => CombatStatPredicateV1::SelectedHandSlotsDiffer,
        }
    }
}

/// A set of clan ids below 64, as a bit mask so the predicate stays `Copy`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClanSetV1(u64);

impl ClanSetV1 {
    pub fn from_ids(ids: &[u32]) -> Option<Self> {
        if ids.is_empty() {
            return None;
        }
        let mut mask = 0_u64;
        for id in ids {
            if *id >= 64 {
                return None;
            }
            mask |= 1 << id;
        }
        Some(Self(mask))
    }

    pub const fn contains(self, clan_id: u32) -> bool {
        clan_id < 64 && self.0 & (1 << clan_id) != 0
    }
}

/// Per-owner clan context for the clan-gated predicates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClanContext {
    owner_effective_clan: u32,
    owner_previous_clan: Option<u32>,
    opponent_hand_clans: u64,
    /// The owner's paid Pillz plus the free pill: what the `Bet` gates compare. Carried
    /// here because this is already the per-owner context every predicate reads.
    owner_pillz_used: u16,
}

/// Public provenance metadata for an admitted post-round effect. The hot path converts this
/// fixed effect into its private typed commit plan; its numeric rule is not configurable.
/// The round scaling a `Growth:` or `Degrowth:` post-round effect carries: the printed
/// amount times the zero-based round plus one, or times four less the zero-based round.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RoundScaleV1 {
    Growth,
    Degrowth,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CombatStatPostRoundEffectV1 {
    /// `Defeat: Recover N Pillz Out Of M`: the loser recovers `max(1, floor((paid + 1) * N
    /// / M))`, where `paid` is the bet plus Fury's three - the Pillz placed on the card.
    RecoverPaidPillzOnDefeat {
        numerator: u16,
        denominator: u16,
    },
    /// `Recover N Pillz Out Of M` and its `Unison :` form: the same recovery for the winner.
    RecoverPaidPillzOnVictory {
        numerator: u16,
        denominator: u16,
    },
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
    /// `+N Pillz Max. M` and `Night: +N Pillz Max. M`: a living winner's own Pillz rise by
    /// `pillz`, never past `maximum`, and an owner already at or above it gains nothing.
    /// Admitted by exact text and shape from the Ability slot.
    GainPillzOnVictoryMax {
        pillz: u16,
        maximum: u16,
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
    /// `+N Life Per Opp. Damage`: the winner's own Life rises by `life_per_damage` for
    /// every point of the *losing* card's final resolved Damage, Fury included.
    GainLifePerOpponentFinalDamageOnVictory {
        life_per_damage: u16,
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
    /// `Brawl: - N Opp. Life Min M`: the winner reduces the opposing player's Life by N per
    /// anti-support count, never below `minimum`.
    ReduceOpponentLifeOnVictoryPerAntiSupport {
        per_count: u16,
        minimum: u16,
    },
    /// `Brawl: -N Opp. Pillz, Min M`: the same reduction on the opposing player's Pillz.
    ReduceOpponentPillzOnVictoryPerAntiSupport {
        per_count: u16,
        minimum: u16,
    },
    /// `Brawl: +N Pillz` and its `Max. M` form: the winner's own Pillz rise by N per
    /// anti-support count, never past `maximum` when that is non-zero.
    GainPillzOnVictoryPerAntiSupport {
        per_count: u16,
        maximum: u16,
    },
    /// `Support: -N Opp. Life, Min M`: the winner reduces the opposing player's Life by N per
    /// distinct character in its own hand sharing its selected card's effective clan, never
    /// below `minimum`.
    ReduceOpponentLifeOnVictoryPerSupport {
        per_count: u16,
        minimum: u16,
    },
    /// `Support: +N Life`: the winner's own Life rises by N per Support count.
    GainLifeOnVictoryPerSupport {
        per_count: u16,
    },
    /// `Support: + N Pillz`: the winner's own Pillz rise by N per Support count.
    GainPillzOnVictoryPerSupport {
        per_count: u16,
    },
    /// `Equalizer: +N Life`: the winner's own Life rises by N per star of the opposing
    /// selected card.
    GainLifeOnVictoryPerOpponentStars {
        per_star: u16,
    },
    /// `Equalizer: +N Pillz`: the same gain on the winner's own Pillz.
    GainPillzOnVictoryPerOpponentStars {
        per_star: u16,
    },
    /// `Killshot: +N Pillz And Life`: an attack at least double the opposing one gives a
    /// living owner N Pillz and N Life.
    GainPillzAndLifeOnKillshot {
        amount: u16,
    },
    /// `Killshot: +N Pillz`: the compound's Pillz half on its own, paid to a living owner.
    GainPillzOnKillshot {
        pillz: u16,
    },
    /// `Killshot: +N Life`, `Killshot: +N Life Max. M` and `Unison: Killshot: +N Life`: the
    /// compound's Life half on its own, paid to a living owner and never past `maximum`
    /// when that is non-zero. The Unison gate is the plan's predicate.
    GainLifeOnKillshot {
        life: u16,
        maximum: u16,
    },
    /// `Killshot: Toxin N, Min M`: the attack ratio latches the plain Toxin, which then pays
    /// in the latching round and every later one exactly as a won Toxin does.
    ToxinOpponentLifeOnKillshot {
        life: u16,
        minimum: u16,
    },
    /// `Defeat: +N Pillz`: a living loser gains N Pillz.
    GainPillzOnDefeat {
        pillz: u16,
    },
    /// `Defeat: +N Pillz And Life`: a living loser gains N Pillz and N Life.
    GainPillzAndLifeOnDefeat {
        amount: u16,
    },
    /// The `Growth:`/`Degrowth:` forms of the plain Victory grammars: the printed amount is
    /// scaled by the round, then paid and clamped as the plain grammar is.
    ReduceOpponentLifeOnVictoryPerRound {
        per_round: u16,
        minimum: u16,
        scale: RoundScaleV1,
    },
    ReduceOpponentPillzOnVictoryPerRound {
        per_round: u16,
        minimum: u16,
        scale: RoundScaleV1,
    },
    GainLifeOnVictoryPerRound {
        per_round: u16,
        scale: RoundScaleV1,
    },
    GainPillzOnVictoryPerRound {
        per_round: u16,
        scale: RoundScaleV1,
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
    /// `Victory Or Defeat : +N Players Life`: whatever the outcome, each living player
    /// gains `life`.
    GainBothPlayersLifeOnVictoryOrDefeat {
        life: u16,
    },
    /// `Victory Or Defeat : +N Players Pillz`: whatever the outcome, both players gain
    /// `pillz`, a knocked-out one included.
    GainBothPlayersPillzOnVictoryOrDefeat {
        pillz: u16,
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
    /// `Defeat: Poison N, Min M`: the same latch, triggered by the owner losing the round
    /// rather than winning it. Ability slot only.
    PoisonOpponentLifeOnDefeat {
        life: u16,
        minimum: u16,
    },
    /// `Toxin N, Min M`: Poison that also pays in its latching round. Ability slot only.
    ToxinOpponentLifeOnVictory {
        life: u16,
        minimum: u16,
    },
    /// `Consume N, Min M`: Toxin on the opposing Pillz. Ability slot only.
    ConsumeOpponentPillzOnVictory {
        pillz: u16,
        minimum: u16,
    },
    /// `Combust N, Min M`: Poison on both opposing Life and Pillz, each floored at M.
    /// Ability slot only.
    CombustOpponentLifeAndPillzOnVictory {
        amount: u16,
        minimum: u16,
    },
    /// `Dope N, Max. M`: Regen on the owner's Pillz, latched by a win. Ability slot only.
    DopePillzOnVictory {
        pillz: u16,
        maximum: u16,
    },
    /// `Unison : +N Pillz And Life`: a living winner gains N Pillz and then N Life.
    GainPillzAndLifeOnVictory {
        amount: u16,
    },
    /// `Victory Or Defeat : +N Pillz`, N of two or more. Ability slot only.
    GainPillzOnVictoryOrDefeat {
        pillz: u16,
    },
    /// `Victory Or Defeat: +N Life Per Damage`. Ability slot only.
    GainLifePerFinalDamageOnVictoryOrDefeat {
        life_per_damage: u16,
    },
    /// `-N Opp. Pillz And Life, Min M`: a winner takes N from each opposing resource, neither
    /// below `minimum`. Ability slot only.
    ReduceOpponentPillzAndLifeOnVictory {
        amount: u16,
        minimum: u16,
    },
    /// `Defeat: Dope N, Max. M`: the same permanent latched by a loss. Ability slot only.
    DopePillzOnDefeat {
        pillz: u16,
        maximum: u16,
    },
    /// `Backlash: - N Life Min M`, M of one or more: the winner's own Life falls by `life`,
    /// never below `minimum`. Ability slot only.
    ReduceOwnLifeOnVictory {
        life: u16,
        minimum: u16,
    },
    /// `Corrupt N Min. M`, M of one or more: whatever the outcome, the owner's own Life falls
    /// by `life`, never below `minimum`. Ability slot only.
    ReduceOwnLife {
        life: u16,
        minimum: u16,
    },
    /// `Defeat: +N Life, Max. M`: a living loser gains `life`, never past `maximum`. Ability
    /// slot only.
    GainLifeOnDefeatMax {
        life: u16,
        maximum: u16,
    },
    /// `Defeat: +N Opp. Pillz`: the losing owner gives the opposing player `pillz`. Ability
    /// slot only.
    GainOpponentPillzOnDefeat {
        pillz: u16,
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
    /// The two selected cards swap their printed values of the stat, in the same phase as a
    /// stat Copy: before any own increase and before any opposing reduction.
    ExchangePrintedCombatStat {
        stat: CombatStatAttributeV1,
    },
    /// `Damage Impose`: the opposing selected card's stat becomes the owner's printed value,
    /// in the Copy phase. Ability slot only, unconditional.
    ImposePrintedCombatStat {
        stat: CombatStatAttributeV1,
    },
    /// While live, the opposing selected card's end-of-round effects on the named resources
    /// are dropped for the round. Ability slot only.
    CancelOpponentResourceModifiers {
        resources: crate::effect_registry::ResourceCancellationV1,
    },
    /// `Tune Out`, while live on either selected card: both Powers become 1 after every
    /// Power modifier, so each Attack is its owner's bet plus one, and no Attack modifier of
    /// either card applies. Damage, Fury and end-of-round effects are untouched. Bonus slot
    /// only - it has only been observed as the Cosmohnuts clan bonus.
    SimplifyAttackToPillz,
    /// Non-stat post-round recovery of the Pillz placed on the losing card. Construction
    /// checks the ratio is a proper fraction and the predicate is unconditional.
    RecoverPaidPillzOnDefeat {
        numerator: u16,
        denominator: u16,
    },
    /// The winning side's recovery, unconditional or under `Unison :`.
    RecoverPaidPillzOnVictory {
        numerator: u16,
        denominator: u16,
    },
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
    /// Capped Victory Pillz, admitted only by the cold structured compiler from the Ability
    /// slot: the winner gains `pillz`, never past `maximum`.
    GainPillzOnVictoryMax {
        pillz: u16,
        maximum: u16,
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
    /// `+N Life Per Opp. Damage`: the winner's own Life rises by `life_per_damage` for
    /// every point of the *losing* card's final resolved Damage, Fury included.
    GainLifePerOpponentFinalDamageOnVictory {
        life_per_damage: u16,
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
    /// Victory-only opponent-Life reduction whose magnitude is the printed amount times the
    /// owner's anti-support count, bound at resolution like Equalizer's stars. Ability slot
    /// only, unconditional.
    ReduceOpponentLifeOnVictoryPerAntiSupport {
        per_count: u16,
        minimum: u16,
    },
    /// The same anti-support-scaled reduction on the opposing player's Pillz.
    ReduceOpponentPillzOnVictoryPerAntiSupport {
        per_count: u16,
        minimum: u16,
    },
    /// The anti-support-scaled own Pillz gain, capped at `maximum` when that is non-zero.
    GainPillzOnVictoryPerAntiSupport {
        per_count: u16,
        maximum: u16,
    },
    /// The post-round `Support:` grammars: the printed amount times the owner's own
    /// effective-clan character count, bound at resolution from the source's Support
    /// context. Ability slot only, unconditional.
    ReduceOpponentLifeOnVictoryPerSupport {
        per_count: u16,
        minimum: u16,
    },
    GainLifeOnVictoryPerSupport {
        per_count: u16,
    },
    GainPillzOnVictoryPerSupport {
        per_count: u16,
    },
    /// The post-round `Equalizer:` own gains: the printed amount times the opposing selected
    /// card's stars. Ability slot only, unconditional.
    GainLifeOnVictoryPerOpponentStars {
        per_star: u16,
    },
    GainPillzOnVictoryPerOpponentStars {
        per_star: u16,
    },
    /// The Killshot compound own gain, Ability slot only.
    GainPillzAndLifeOnKillshot {
        amount: u16,
    },
    /// The compound's halves on their own and the ratio-latched Toxin, Ability slot only.
    /// Only the uncapped Life gain may carry a predicate, and only the `Unison:` gate.
    GainPillzOnKillshot {
        pillz: u16,
    },
    GainLifeOnKillshot {
        life: u16,
        maximum: u16,
    },
    ToxinOpponentLifeOnKillshot {
        life: u16,
        minimum: u16,
    },
    /// The Defeat own Pillz gain and its compound, Ability slot only.
    GainPillzOnDefeat {
        pillz: u16,
    },
    GainPillzAndLifeOnDefeat {
        amount: u16,
    },
    /// The round-scaled Victory grammars, Ability slot only.
    ReduceOpponentLifeOnVictoryPerRound {
        per_round: u16,
        minimum: u16,
        scale: RoundScaleV1,
    },
    ReduceOpponentPillzOnVictoryPerRound {
        per_round: u16,
        minimum: u16,
        scale: RoundScaleV1,
    },
    GainLifeOnVictoryPerRound {
        per_round: u16,
        scale: RoundScaleV1,
    },
    GainPillzOnVictoryPerRound {
        per_round: u16,
        scale: RoundScaleV1,
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
    /// The both-players Victory Or Defeat gains, Ability slot only.
    GainBothPlayersLifeOnVictoryOrDefeat {
        life: u16,
    },
    GainBothPlayersPillzOnVictoryOrDefeat {
        pillz: u16,
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
    /// Poison latched by a lost round instead of a won one.
    PoisonOpponentLifeOnDefeat {
        life: u16,
        minimum: u16,
    },
    /// Poison's immediate sibling. With Min 0 the repeat itself can end the match.
    ToxinOpponentLifeOnVictory {
        life: u16,
        minimum: u16,
    },
    /// The opposing player loses `pillz` Pillz from the latching round on while above
    /// `minimum`, whether or not that player is still living.
    ConsumeOpponentPillzOnVictory {
        pillz: u16,
        minimum: u16,
    },
    /// From the round after the latch, the opposing player loses `amount` Life and `amount`
    /// Pillz, each only while above `minimum`. With Min 0 the Life half can end the match.
    CombustOpponentLifeAndPillzOnVictory {
        amount: u16,
        minimum: u16,
    },
    /// From the latching round on, the owner gains `pillz` Pillz while below `maximum`,
    /// never past it, whether or not the owner is still living. Latched by a win, or by a
    /// loss for the Defeat form.
    DopePillzOnVictory {
        pillz: u16,
        maximum: u16,
    },
    /// The Unison Victory compound: Ability slot only, under `OwnerHandUnison`.
    GainPillzAndLifeOnVictory {
        amount: u16,
    },
    /// The owner gains `pillz` Pillz whatever the outcome, a knocked-out owner included.
    GainPillzOnVictoryOrDefeat {
        pillz: u16,
    },
    /// A living owner gains `life_per_damage` Life per point of its own final Damage,
    /// whatever the outcome.
    GainLifePerFinalDamageOnVictoryOrDefeat {
        life_per_damage: u16,
    },
    /// The winner takes `amount` from each opposing resource, neither below `minimum`.
    ReduceOpponentPillzAndLifeOnVictory {
        amount: u16,
        minimum: u16,
    },
    DopePillzOnDefeat {
        pillz: u16,
        maximum: u16,
    },
    /// Backlash: the winner's own Life falls by `life`, never below `minimum`, and an owner
    /// at or below it is left alone. Ability slot only, unconditional, `minimum` at least 1.
    ReduceOwnLifeOnVictory {
        life: u16,
        minimum: u16,
    },
    /// Corrupt: whatever the outcome, the owner's own Life falls by `life`, never below
    /// `minimum`, and an owner at or below it is left alone. Ability slot only,
    /// unconditional, `minimum` at least 1.
    ReduceOwnLife {
        life: u16,
        minimum: u16,
    },
    /// Defeat Life with Heal's cap: a living loser gains `life`, never past `maximum`, and an
    /// owner already at or above it gains nothing. Ability slot only, unconditional.
    GainLifeOnDefeatMax {
        life: u16,
        maximum: u16,
    },
    /// The opposing player gains `pillz` when the owner loses, whether or not the round has
    /// knocked the owner out. Ability slot only, unconditional.
    GainOpponentPillzOnDefeat {
        pillz: u16,
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
    AmbiguousOculusClanGate,
    /// A `Damage Impose` facing an opposing `Cancel Opp. Damage Modif.` (or a Copy that
    /// could adopt one): no captured round shows which of the two wins.
    UnmodelledImposeContext,
    CappedIncrease,
    CompoundPredicateAndMagnitude,
    ConditionalBonus,
    ConditionalControl,
    IncompatibleBounds,
    InvalidModifierDirection,
    RecoveryRatio,
    RecoveryPredicate,
    RecoverySource,
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
    VictoryPillzMaxSource,
    VictoryPillzMaxMagnitude,
    VictoryPillzMaxPredicate,
    /// Capped Victory Pillz beside an opposing effect that can write its owner's Pillz in
    /// the round it pays, or an opposing Copy: the cap makes any cross-owner order
    /// observable, and 1093173/1 shows the server's order is not the engine's.
    CappedVictoryPillzAgainstUnpinnedEffect,
    /// A revision-70 clan-gated end-of-round source beside an opposing write to the same
    /// resource that can land in the round it pays, or beside an opposing Copy: 1093173/1
    /// shows the server's cross-owner order is not the engine's, and no round shows a clan
    /// gate judged for a copier.
    ClanGatedPostRoundAgainstUnpinnedEffect,
    /// A revision-73 `Versus` or `After` end-of-round source beside a write to the same
    /// resource that can land in the round it pays - an opposing one, or an order-sensitive
    /// one from the card's other slot or an own latch - or beside an opposing Copy.
    HandClanGatedPostRoundAgainstUnpinnedEffect,
    /// A revision-73 `Unison :` Toxin or Consume beside another latch of its family that
    /// could target the same player. The server prints that a second latch replaces the
    /// first while the engine stacks them, and which it is remains an open decision.
    UnisonLatchAgainstSameFamilyLatch,
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
    /// Backlash Life: card abilities only, a positive magnitude, a Min of at least 1 (a `Min
    /// 0` record could knock its own owner out, which no round shows) and no predicate.
    BacklashLifeSource,
    BacklashLifeMagnitude,
    BacklashLifePredicate,
    /// Backlash Life beside an effect on its owner's Life that can land in the round it
    /// pays - an opposing one written on the opposing loss, the Backlash card's other slot,
    /// or an own latched permanent - or beside an opposing Copy. 1093173/1 shows the
    /// server's cross-owner order is not the engine's, and no round shows the order within
    /// one owner.
    BacklashLifeAgainstUnpinnedEffect,
    /// Corrupt: card abilities only, a positive magnitude, a Min of at least 1 (a `Min 0`
    /// record could knock its own owner out, which no round shows) and no predicate.
    CorruptLifeSource,
    CorruptLifeMagnitude,
    CorruptLifePredicate,
    /// Corrupt beside any other effect on its owner's Life that can land in the same round -
    /// an opposing floor or both-players gain on either outcome, a latched Poison included,
    /// the Corrupt card's other slot or an own latched permanent - or beside an opposing Life
    /// canceller or an opposing Copy. Its two pinned rounds have no other writer on that
    /// Life, and no round shows a canceller meeting it.
    CorruptLifeAgainstUnpinnedEffect,
    /// The capped Defeat Life beside an opposing floor on its owner's Life written on the
    /// opposing win, a same-owner writer of that Life, or an opposing Copy: the cap makes
    /// either order observable.
    CappedDefeatLifeAgainstUnpinnedEffect,
    /// The Defeat opposing Pillz gift beside a capped own Pillz gain of its target that pays
    /// on the target's win, a same-owner floor on the target's Pillz, or an opposing Copy.
    DefeatOpponentPillzGiftAgainstUnpinnedEffect,
    ReanimateLifeSource,
    ReanimateLifeMagnitude,
    ReanimateLifePredicate,
    VictoryOrDefeatLifeIdentity,
    VictoryOrDefeatLifeEffect,
    VictoryOrDefeatLifePredicate,
    EqualizerOpponentLifeIdentity,
    EqualizerOpponentLifeEffect,
    EqualizerOpponentLifePredicate,
    BrawlPostRoundSource,
    BrawlPostRoundMagnitude,
    BrawlPostRoundPredicate,
    SupportPostRoundSource,
    SupportPostRoundMagnitude,
    SupportPostRoundPredicate,
    /// The Equalizer opponent-Life grammar beyond its two reviewed identities, and the
    /// Equalizer own gains.
    EqualizerPostRoundSource,
    EqualizerPostRoundMagnitude,
    EqualizerPostRoundPredicate,
    /// A `Stop:` source faces a hand that could stop its owner's ability, the one case the
    /// projection does not model.
    StopTriggeredAgainstStopAbility,
    /// A `Cards` plan outside the one admitted shape: a fixed Damage or Attack change from a
    /// card ability, with no condition, no cap, and a Min exactly when it is a decrease.
    BothCardsModifierShape,
    /// `Tune Out` from any slot but the clan Bonus.
    AttackSimplificationSource,
    /// `Tune Out` in a match that also holds a Killshot, which reads the Attacks it replaces,
    /// an opposing `Cancel Opp. Power/Attack Modif.`, whose effect on it no round has shown,
    /// a Power reduction that could reach 0 before Power is set to 1, or an opposing Copy.
    AttackSimplificationAgainstUnpinnedEffect,
    /// A `Cards` modifier facing an opposing cancel of its stat or an opposing Copy, neither
    /// of which any round has shown meeting one.
    BothCardsModifierAgainstUnpinnedEffect,
    /// A Killshot in a match where both final Attacks could reach 0: the ratio then holds
    /// for the side that loses the tie, and no round shows whether that pays.
    KillshotAgainstZeroAttacks,
    /// `Consume` or `Combust` facing an opposing end-of-round effect on a resource it
    /// floors, or an opposing Copy. The two players' effects then meet on one resource, and
    /// their order at a binding floor is unpinned (1093173/1 shows the server's is not the
    /// engine's for fresh effects).
    PillzPermanentAgainstOpposingResourceEffect,
    /// A single-stat Protection facing an opposing effect whose meeting with it no captured
    /// round has shown, or an opposing Copy.
    SingleStatProtectionAgainstUnpinnedEffect,
    /// A Recover facing an opposing reduction of its owner's Pillz towards a floor, or an
    /// opposing Copy.
    RecoveryAgainstUnpinnedEffect,
    /// A Dope beside any other effect on its owner's Pillz, or an opposing Copy: its cap
    /// makes the order observable, and no round pins it.
    DopeAgainstUnpinnedEffect,
    UnisonPillzAndLifeSource,
    UnisonPillzAndLifeMagnitude,
    UnisonPillzAndLifePredicate,
    /// A Unison Life gain beside an effect on its owner's Life - or, for the compound, on
    /// its owner's Pillz - whose order against it no round pins, or an opposing Copy.
    UnisonGainAgainstUnpinnedEffect,
    VictoryOrDefeatGainSource,
    VictoryOrDefeatGainMagnitude,
    VictoryOrDefeatGainPredicate,
    VictoryOpponentPillzAndLifeSource,
    VictoryOpponentPillzAndLifeMagnitude,
    VictoryOpponentPillzAndLifePredicate,
    /// A Victory Or Defeat own gain beside an effect on its owner's resource whose order
    /// against it no round pins, or an opposing Copy.
    VictoryOrDefeatGainAgainstUnpinnedEffect,
    /// The opposing compound beside an opposing effect on the resources it floors that can
    /// land in the same round, or an opposing Copy: 1093173/1 shows the engine's order is not
    /// the server's for exactly this compound.
    OpponentPillzAndLifeAgainstUnpinnedEffect,
    /// A source an opposing Bonus-slot Copy - the Oblivion clan bonus - could run from its
    /// Bonus slot where no round shows it there: Anita's identity-locked conversion, or a
    /// conditional Stop, which the validator admits from the Ability slot only.
    BonusSlotCopyOfUnpinnedSource,
    KillshotPillzAndLifeSource,
    DefeatPillzSource,
    BothPlayersGainSource,
    BothPlayersGainMagnitude,
    BothPlayersGainPredicate,
    /// `Victory Or Defeat : +N Players Life`, whose effect on a player the round has knocked
    /// out no captured round shows.
    BothPlayersLifeGainAgainstKnockout,
    DefeatPillzMagnitude,
    DefeatPillzPredicate,
    RoundScaledPostRoundSource,
    RoundScaledPostRoundMagnitude,
    RoundScaledPostRoundPredicate,
    KillshotPillzAndLifeMagnitude,
    KillshotPillzAndLifePredicate,
    /// The Killshot own gains and the Killshot Toxin latch: card abilities only, a positive
    /// magnitude, and no predicate except `Unison:` on the uncapped Life gain.
    KillshotPostRoundSource,
    KillshotPostRoundMagnitude,
    KillshotPostRoundPredicate,
    ResourceCancellationSource,
    /// A resource canceller faces an opposing effect whose cancellation no captured round
    /// has shown: a permanent, a compound, a both-players reduction, a Copy, or - for Pillz
    /// and Life - any Pillz effect.
    ResourceCancellationAgainstUnpinnedEffect,
    /// A `/ Life Lost` magnitude whose reader's Life could rise during the match: an own
    /// Life gain, an opposing one an own Copy could adopt, or either of those for an
    /// opposing Copy that could adopt the source itself. Every captured round reads an
    /// owner whose Life has only fallen, which cannot tell the net shortfall from the sum
    /// of every point lost.
    LifeLostOwnerLifeCanRise,
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
                validate_stop_triggered_context(
                    player,
                    slot,
                    &spec.cards[player],
                    &spec.cards[player.other()],
                )?;
                validate_clan_gate_context(player, slot, &spec)?;
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
        let players = self.base_rules.position().players;
        let life = ByPlayer::new(players[PlayerId::P1].life, players[PlayerId::P2].life);
        let pillz = ByPlayer::new(players[PlayerId::P1].pillz, players[PlayerId::P2].pillz);
        let initial = &self.spec.base_rules.players;
        let pillz_lost = ByPlayer::new(
            initial[PlayerId::P1]
                .initial_pillz
                .saturating_sub(players[PlayerId::P1].pillz),
            initial[PlayerId::P2]
                .initial_pillz
                .saturating_sub(players[PlayerId::P2].pillz),
        );
        let life_lost = ByPlayer::new(
            initial[PlayerId::P1]
                .initial_life
                .saturating_sub(players[PlayerId::P1].life),
            initial[PlayerId::P2]
                .initial_life
                .saturating_sub(players[PlayerId::P2].life),
        );
        let prepared = prepare_combat_stat_diagnostic(
            validated,
            &self.spec.cards,
            input.first_mover,
            rounds_played,
            previous_round_winner,
            self.spec.base_rules.night,
            life,
            pillz,
            pillz_lost,
            life_lost,
            self.spec
                .base_rules
                .players
                .map(|player| player.hand.map(|card| card.clan_id)),
            self.base_rules.position().previous_round_slots,
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

/// True for every effect whose magnitude is its owner's Support count - the distinct
/// characters in the owner's hand sharing the owner's selected card's effective clan - so
/// that an ability carrying it must carry that count as its Support context. The combat-stat
/// Support abilities and the post-round `Support:` grammars; catalog and replay preparation
/// ask this same question, so the three can never disagree about which plans need a count.
pub(crate) const fn effect_reads_support_count(effect: CombatStatEffectV1) -> bool {
    matches!(
        effect,
        CombatStatEffectV1::ModifyCombatStat {
            multiplier: CombatStatMagnitudeV1::SourceBonusSupport,
            ..
        } | CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerSupport { .. }
            | CombatStatEffectV1::GainLifeOnVictoryPerSupport { .. }
            | CombatStatEffectV1::GainPillzOnVictoryPerSupport { .. }
    )
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
        CombatStatSourcePlanV1::Execute { effect, .. } if effect_reads_support_count(effect)
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
/// A `Stop:` source fires only when an opposing source stops its owner's ability, which no
/// captured round has shown. The projection models it as never firing, which is sound only
/// while nothing opposite can stop that ability: a `Stop Opp. Ability` in any opposing slot,
/// under any predicate, or a Copy that could adopt one. A match with either is refused
/// rather than executed on an unpinned rule.
fn validate_stop_triggered_context(
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
        if let Some(reason) = unmodelled_source_context(plan, hand_slot, own, opponent) {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_plan_id(plan).unwrap_or(0),
                reason,
            ));
        }
    }
    Ok(())
}

/// The sources admitted only where their context is one the corpus has pinned, and why a
/// given match is not: a `Stop:` source facing anything that could stop its owner's
/// ability, a resource canceller facing an effect whose cancellation is unpinned, a
/// `/ Life Lost` magnitude whose owner's Life could rise, and `Tune Out` meeting a
/// Killshot or an opposing Power/Attack cancel. Construction refuses them (catalog as an
/// unsupported source, replay as a selected hazard, the engine as an invalid plan).
pub(crate) fn unmodelled_source_context(
    plan: CombatStatSourcePlanV1,
    hand_slot: HandSlot,
    own: &[CombatStatCardPlanV1; HAND_SIZE],
    opponent: &[CombatStatCardPlanV1; HAND_SIZE],
) -> Option<InvalidCombatStatPlanReasonV1> {
    match plan {
        // The Oblivion clan bonus adopts the opposing selected card's ability and runs it from
        // its own Bonus slot. That is pinned for plain effects and for an unconditional Stop
        // (924669/3), but no round shows Anita's conversion there - its identity lock names
        // the printed Ability - or a conditional Stop, which the validator admits from the
        // Ability slot only. An ability-slot Copy of Anita is older and left as it was.
        CombatStatSourcePlanV1::Execute {
            source_id,
            predicate,
            effect,
        } if opposing_bonus_copy_can_take(plan, own, opponent)
            && (source_id == 274
                || (matches!(
                    effect,
                    CombatStatEffectV1::StopOpponentAbility | CombatStatEffectV1::StopOpponentBonus
                ) && predicate != CombatStatPredicateV1::Always)) =>
        {
            Some(InvalidCombatStatPlanReasonV1::BonusSlotCopyOfUnpinnedSource)
        }
        CombatStatSourcePlanV1::Execute {
            effect:
                CombatStatEffectV1::ModifyCombatStat {
                    multiplier: CombatStatMagnitudeV1::OwnerLifeLost,
                    ..
                },
            ..
        } if life_lost_reader_life_can_rise(own, opponent) => {
            Some(InvalidCombatStatPlanReasonV1::LifeLostOwnerLifeCanRise)
        }
        CombatStatSourcePlanV1::Execute {
            predicate: CombatStatPredicateV1::OwnerAbilityStopped,
            ..
        } if opponent_can_stop_an_ability(opponent) => {
            Some(InvalidCombatStatPlanReasonV1::StopTriggeredAgainstStopAbility)
        }
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::CancelOpponentResourceModifiers { resources },
            ..
        } if opponent_defeats_resource_cancellation(resources, opponent) => {
            Some(InvalidCombatStatPlanReasonV1::ResourceCancellationAgainstUnpinnedEffect)
        }
        // Under `Tune Out` the server reports each Attack as its bet, but no round shows a
        // Killshot judged on those Attacks, an opposing cancel of Power or Attack modifiers
        // meeting it, a Power reduction reaching 0 before Power is set to 1 (the reference
        // applies the two in another order), or a Copy adopting it. Both sides of each pair
        // are checked, so whichever source the caller visits first is refused.
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::SimplifyAttackToPillz,
            ..
        } if source_plans(opponent).any(reads_simplified_attack)
            || source_plans(own)
                .chain(source_plans(opponent))
                .any(reduces_power_to_zero)
            || hand_has_copy(opponent) =>
        {
            Some(InvalidCombatStatPlanReasonV1::AttackSimplificationAgainstUnpinnedEffect)
        }
        // `Consume` and `Combust` floor the opposing player's Pillz (and Life) every round
        // after they latch. Wherever that player's own end-of-round effects also write the
        // resource, the two players' effects meet on it, and which lands first decides the
        // result exactly when the floor binds. No round pins that order for a permanent, and
        // 1093173/1 shows the server's order for fresh effects is not the engine's, so such
        // a match is refused - as is one where an opposing Copy could adopt a writer.
        CombatStatSourcePlanV1::Execute {
            effect:
                effect @ (CombatStatEffectV1::ConsumeOpponentPillzOnVictory { .. }
                | CombatStatEffectV1::CombustOpponentLifeAndPillzOnVictory { .. }),
            ..
        } if hand_has_copy(opponent)
            || source_plans(opponent).any(|plan| writes_floored_resource(plan, effect)) =>
        {
            Some(InvalidCombatStatPlanReasonV1::PillzPermanentAgainstOpposingResourceEffect)
        }
        // `Damage Impose` writes the owner's printed Damage onto the opposing card in the
        // Copy phase. No round shows one meeting an opposing Cancel of Damage modifiers, or a
        // Copy that could adopt one, so either in the opposing hand refuses it.
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::ImposePrintedCombatStat { .. },
            ..
        } if hand_has_copy(opponent) || source_plans(opponent).any(cancels_damage_modifiers) => {
            Some(InvalidCombatStatPlanReasonV1::UnmodelledImposeContext)
        }
        // Recover raises its owner's Pillz with no cap, which commutes with every other gain
        // but not with an opposing reduction towards a floor: the two orders differ exactly
        // when the floor binds, and no round pins the server's (1093173/1 shows it is not the
        // engine's P1-then-P2 for a Pillz gain against a floored reduction; 1091644/1 meets
        // one where both orders agree). Nor has any round shown a Copy taking a Recover. So
        // either in the opposing hand refuses it - except revision 8's `Defeat: Recover 2
        // Pillz Out Of 3`, admitted before either question was found and left as it was.
        CombatStatSourcePlanV1::Execute {
            effect:
                effect @ (CombatStatEffectV1::RecoverPaidPillzOnDefeat { .. }
                | CombatStatEffectV1::RecoverPaidPillzOnVictory { .. }),
            ..
        } if effect != REVISION_8_RECOVERY
            && (opposing_copy_can_take(plan, own, opponent)
                || source_plans(opponent).any(floors_opposing_pillz)) =>
        {
            Some(InvalidCombatStatPlanReasonV1::RecoveryAgainstUnpinnedEffect)
        }
        // The opposing compound floors both opposing resources on its owner's win. The
        // target's own writes to either that can land in that round - on the target's loss,
        // or every round for a permanent - meet it in an order the server has shown the
        // engine getting wrong for this very compound (1093173/1), so the match is refused,
        // as it is beside an opposing Copy.
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::ReduceOpponentPillzAndLifeOnVictory { .. },
            ..
        } if opposing_copy_can_take(plan, own, opponent)
            || source_plans(opponent).any(|opposing| {
                let life = life_writes(opposing);
                write_outcomes(opposing).on_loss
                    && (pillz_writes(opposing).own_gain
                        || life.own_gain
                        || life.own_order_sensitive)
            }) =>
        {
            Some(InvalidCombatStatPlanReasonV1::OpponentPillzAndLifeAgainstUnpinnedEffect)
        }
        // Capped Victory Pillz pays on its owner's win, and its cap makes the order against
        // any other write to its owner's Pillz observable: an opposing gain or floor landing
        // first moves the value the cap reads. No round shows one beside such a write, and
        // 1093173/1 shows the server's cross-owner order is not the engine's P1-then-P2, so a
        // match is refused wherever an opposing effect can write the owner's Pillz in the
        // owner's winning round - on the opposing loss, or every round for a permanent - or
        // an opposing Copy could take the gain, or import an own write onto the owner's Pillz
        // (`pillz_writes` reports nothing for a Copy). The owner's own writes are not refused:
        // Argos (1093451) pins the owner's bonus before its ability.
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::GainPillzOnVictoryMax { .. },
            ..
        } if opposing_copy_can_take(plan, own, opponent)
            || (hand_has_copy(opponent)
                && source_plans(own).any(|own_plan| {
                    let writes = pillz_writes(own_plan);
                    writes.opposing_gain || writes.opposing_floor
                }))
            || source_plans(opponent).any(|opposing| {
                let writes = pillz_writes(opposing);
                write_outcomes(opposing).on_loss && (writes.opposing_gain || writes.opposing_floor)
            }) =>
        {
            Some(InvalidCombatStatPlanReasonV1::CappedVictoryPillzAgainstUnpinnedEffect)
        }
        // Backlash floors its owner's own Life on the owner's win. Any other write to that Life
        // landing in the same round meets it at the floor, and no round pins either order:
        // 1093173/1 shows the server's cross-owner order is not the engine's P1-then-P2, and
        // 946913/0 shows Uuber's opposing floor beside it without either floor binding. So a
        // match is refused where an opposing effect can write the owner's Life on the opposing
        // loss - a floor, or a both-players gain, a latched permanent included since it writes
        // on every outcome - where the Backlash card's other slot or an own latched permanent
        // writes the owner's Life, or where the opposing hand holds a Copy, which could take
        // the Backlash or import a writer (`life_writes` reports nothing for a Copy).
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::ReduceOwnLifeOnVictory { .. },
            ..
        } if hand_has_copy(opponent)
            || source_plans(opponent).any(|opposing| {
                write_outcomes(opposing).on_loss
                    && (life_writes(opposing).opposing_floor
                        || life_beneficiary(opposing) == Some(LifeBeneficiaryV1::Both))
            })
            || same_owner_meets(plan, own, writes_own_life) =>
        {
            Some(InvalidCombatStatPlanReasonV1::BacklashLifeAgainstUnpinnedEffect)
        }
        // Corrupt floors its owner's own Life whatever the round did, so any other write to
        // that Life in the same round meets it at the floor on some outcome. Its two pinned
        // rounds, 1065308/2 and 1066210/2, have no other writer on that Life, and 1093173/1
        // shows the cross-owner order is not the engine's. So a match is refused where an
        // opposing effect floors the owner's Life or raises both players' - on either outcome,
        // a latched Poison or Toxin included - where the Corrupt card's other slot or an own
        // latched permanent writes the owner's Life, where the opposing hand holds a Life
        // canceller, which no round shows meeting Corrupt, or where it holds a Copy, which
        // could take Corrupt or import a writer.
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::ReduceOwnLife { .. },
            ..
        } if hand_has_copy(opponent)
            || source_plans(opponent).any(|opposing| {
                life_writes(opposing).opposing_floor
                    || life_beneficiary(opposing) == Some(LifeBeneficiaryV1::Both)
                    || matches!(
                        opposing,
                        CombatStatSourcePlanV1::Execute {
                            effect: CombatStatEffectV1::CancelOpponentResourceModifiers { .. },
                            ..
                        }
                    )
            })
            || same_owner_meets(plan, own, writes_own_life) =>
        {
            Some(InvalidCombatStatPlanReasonV1::CorruptLifeAgainstUnpinnedEffect)
        }
        // The capped Defeat Life reads its owner's Life when it pays on the owner's loss, so
        // any other write to that Life in the same round moves the value the cap reads. The
        // cross-owner order is 1093173/1's question, so an opposing floor written on the
        // opposing win - or a both-players gain - refuses it, as do the card's other slot or an
        // own latched permanent writing the owner's Life, and an opposing Copy.
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::GainLifeOnDefeatMax { .. },
            ..
        } if hand_has_copy(opponent)
            || source_plans(opponent).any(|opposing| {
                write_outcomes(opposing).on_win
                    && (life_writes(opposing).opposing_floor
                        || life_beneficiary(opposing) == Some(LifeBeneficiaryV1::Both))
            })
            || same_owner_meets(plan, own, writes_own_life) =>
        {
            Some(InvalidCombatStatPlanReasonV1::CappedDefeatLifeAgainstUnpinnedEffect)
        }
        // The gift raises its target's Pillz on the owner's loss, the target's win. An uncapped
        // gain commutes with the target's own uncapped gains, but not with a capped one that can
        // pay on that win (Victory Max, Brawl Max, a Dope latch), and not with a floor the gift's
        // owner puts on the same Pillz from the card's other slot or a latched permanent. Nor
        // has any round shown a Copy taking it.
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::GainOpponentPillzOnDefeat { .. },
            ..
        } if hand_has_copy(opponent)
            || source_plans(opponent).any(|opposing| {
                write_outcomes(opposing).on_win && pillz_writes(opposing).own_capped
            })
            || same_owner_meets(plan, own, floors_opposing_pillz) =>
        {
            Some(InvalidCombatStatPlanReasonV1::DefeatOpponentPillzGiftAgainstUnpinnedEffect)
        }
        // Revision 70's clan-gated end-of-round sources rest on one firing round each, or on
        // none, so they are admitted only where the order question 1093173/1 raised cannot
        // arise and no Copy is in the opposing hand. `Consume` under its compound gate is
        // refused by the `Consume`/`Combust` arm above whatever its predicate.
        CombatStatSourcePlanV1::Execute {
            predicate: CombatStatPredicateV1::OwnerClanIn(_),
            effect,
            ..
        } if clan_gated_post_round_meets_unpinned_effect(effect, opponent) => {
            Some(InvalidCombatStatPlanReasonV1::ClanGatedPostRoundAgainstUnpinnedEffect)
        }
        // Revision 73's `Versus` and `After` end-of-round sources are newly admitted writes, so
        // they carry the 1093173/1 order rule as new effects must, and no round shows either
        // gate judged for a copier.
        CombatStatSourcePlanV1::Execute {
            predicate:
                CombatStatPredicateV1::OpponentHandHasClan(_)
                | CombatStatPredicateV1::OwnerPreviousCardClanIn(_),
            effect,
            ..
        } if hand_clan_gated_post_round_meets_unpinned_effect(plan, effect, own, opponent) => {
            Some(InvalidCombatStatPlanReasonV1::HandClanGatedPostRoundAgainstUnpinnedEffect)
        }
        // Revision 73's `Unison :` latches. The server prints that a second Poison or Toxin
        // (or a second Consume) replaces the first, while the engine stacks them, and no round
        // separates the two (docs/replay-triage.md, "Same-family permanents", an open
        // decision). So the new forms are admitted only where no second latch of their family
        // could target the same player: none elsewhere in the owner's hand, none an own Copy
        // could import from the opposing hand, and no opposing Copy that could take the
        // Unison latch to the other side.
        CombatStatSourcePlanV1::Execute {
            predicate: CombatStatPredicateV1::OwnerHandUnison,
            effect:
                effect @ (CombatStatEffectV1::ToxinOpponentLifeOnVictory { .. }
                | CombatStatEffectV1::ConsumeOpponentPillzOnVictory { .. }),
            ..
        } if opposing_copy_can_take(plan, own, opponent)
            || source_plans(own)
                .filter(|&own_plan| same_latch_family(own_plan, effect))
                .count()
                > 1
            || (hand_has_copy(own)
                && source_plans(opponent).any(|opposing| same_latch_family(opposing, effect))) =>
        {
            Some(InvalidCombatStatPlanReasonV1::UnisonLatchAgainstSameFamilyLatch)
        }
        // The Victory Or Defeat own gains are uncapped and pay whatever the outcome, so an
        // opposing floor on their resource, an own cap on it, or a Copy meets them in an order
        // no round pins.
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::GainPillzOnVictoryOrDefeat { .. },
            ..
        } if opposing_copy_can_take(plan, own, opponent)
            || source_plans(opponent).any(floors_opposing_pillz)
            || source_plans(own).any(|own_plan| pillz_writes(own_plan).own_capped) =>
        {
            Some(InvalidCombatStatPlanReasonV1::VictoryOrDefeatGainAgainstUnpinnedEffect)
        }
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::GainLifePerFinalDamageOnVictoryOrDefeat { .. },
            ..
        } if opposing_copy_can_take(plan, own, opponent)
            || source_plans(opponent).any(|opposing| life_writes(opposing).opposing_floor)
            || source_plans(own).any(|own_plan| life_writes(own_plan).own_order_sensitive) =>
        {
            Some(InvalidCombatStatPlanReasonV1::VictoryOrDefeatGainAgainstUnpinnedEffect)
        }
        // The Unison Life gains are uncapped, so they commute with other uncapped gains, but
        // an opposing floor on the resource, an own cap or revival on it, or a Copy meets
        // them in an order no round pins. The compound meets both resources. Since revision
        // 73 an opposing floor counts only where it can land in a round the gain pays
        // (`opposing_floor_can_meet_unison_gain`).
        CombatStatSourcePlanV1::Execute {
            effect: effect @ CombatStatEffectV1::GainLifeOnDefeat { .. },
            predicate: CombatStatPredicateV1::OwnerHandUnison,
            ..
        }
        | CombatStatSourcePlanV1::Execute {
            effect: effect @ CombatStatEffectV1::GainPillzAndLifeOnVictory { .. },
            ..
        } if opposing_copy_can_take(plan, own, opponent)
            || opponent.iter().enumerate().any(|(index, card)| {
                [card.ability, card.bonus].into_iter().any(|opposing| {
                    life_writes(opposing).opposing_floor
                        && opposing_floor_can_meet_unison_gain(
                            opposing,
                            index == hand_slot.index(),
                            effect,
                        )
                })
            })
            || source_plans(own).any(|own_plan| life_writes(own_plan).own_order_sensitive)
            || (hand_has_copy(own)
                && source_plans(opponent)
                    .any(|opposing| life_writes(opposing).own_order_sensitive))
            || (matches!(effect, CombatStatEffectV1::GainPillzAndLifeOnVictory { .. })
                && source_plans(opponent).any(floors_opposing_pillz)) =>
        {
            Some(InvalidCombatStatPlanReasonV1::UnisonGainAgainstUnpinnedEffect)
        }
        // Dope's cap makes its order against any other effect on the owner's Pillz
        // observable: an own gain landing first leaves less room below the Max, and an
        // opposing floor or gain moves the value the cap reads. No round shows a Dope beside
        // another such effect, so the match is refused wherever one could meet it - another
        // own gain in the hand (or one an own Copy could take from the opposing hand), any
        // opposing write to the owner's Pillz, or an opposing Copy of the Dope's slot.
        CombatStatSourcePlanV1::Execute {
            effect:
                CombatStatEffectV1::DopePillzOnVictory { .. }
                | CombatStatEffectV1::DopePillzOnDefeat { .. },
            ..
        } if opposing_copy_can_take(plan, own, opponent)
            || source_plans(own)
                .filter(|&own_plan| pillz_writes(own_plan).own_gain)
                .count()
                > 1
            || (hand_has_copy(own)
                && source_plans(opponent).any(|opposing| pillz_writes(opposing).own_gain))
            || source_plans(opponent).any(|opposing| {
                let writes = pillz_writes(opposing);
                writes.opposing_gain || writes.opposing_floor
            }) =>
        {
            Some(InvalidCombatStatPlanReasonV1::DopeAgainstUnpinnedEffect)
        }
        // The single-stat Protections have seven selected rounds between them, and each shows
        // one only leaving a reduction of another stat alone: an Attack cut on `Protection:
        // Power` (948108/0, 964088/0, 925204/2) and on `Protection : Damage` (947121/1), and a
        // Power And Damage cut on `Protection: Attack` (1131208/1). None shows one meeting a
        // change to the stat it names, or `Protection: Power` and `Protection : Damage`
        // meeting a change to either of the pair, so a match where the opposing hand could
        // bring one is refused - as is an opposing Copy, which could take the owner's own
        // sources, or the Protection itself, to the other side.
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::ProtectOwnCombatStat { stat },
            ..
        } if stat != CombatStatAttributeV1::PowerAndDamage
            && (hand_has_copy(opponent)
                || source_plans(opponent).any(|plan| meets_unpinned_protection(plan, stat))) =>
        {
            Some(InvalidCombatStatPlanReasonV1::SingleStatProtectionAgainstUnpinnedEffect)
        }
        // A `Cards` modifier has been seen on both cards in nine rounds, but never facing an
        // opposing cancel of its stat or an opposing Copy.
        CombatStatSourcePlanV1::Execute {
            effect:
                CombatStatEffectV1::ModifyCombatStat {
                    side: CombatStatAffectedSideV1::Both,
                    stat,
                    ..
                },
            ..
        } if hand_has_copy(opponent)
            || source_plans(opponent).any(|plan| cancels_modifiers_of(plan, stat)) =>
        {
            Some(InvalidCombatStatPlanReasonV1::BothCardsModifierAgainstUnpinnedEffect)
        }
        CombatStatSourcePlanV1::Execute { effect, .. }
            if is_killshot(effect)
                && source_plans(own)
                    .chain(source_plans(opponent))
                    .any(simplifies_attack) =>
        {
            Some(InvalidCombatStatPlanReasonV1::AttackSimplificationAgainstUnpinnedEffect)
        }
        // A Killshot reads `attack >= 2 x opposing attack`, which holds at 0 against 0 for
        // whichever side then loses the tie. Revision 38 reads that as paying, the reference
        // would not (its Life and Pillz modifiers default to a win), and no round separates
        // them - so a match where both final Attacks could reach 0, by a Min 0 Attack or
        // Power reduction on each side or by a `Cards` one that reduces both, is refused.
        CombatStatSourcePlanV1::Execute { effect, .. }
            if is_killshot(effect)
                && ((source_plans(own).any(zeroes_opposing_attack)
                    && source_plans(opponent).any(zeroes_opposing_attack))
                    || source_plans(own)
                        .chain(source_plans(opponent))
                        .any(zeroes_both_attacks)) =>
        {
            Some(InvalidCombatStatPlanReasonV1::KillshotAgainstZeroAttacks)
        }
        _ => None,
    }
}

/// Whether a revision-70 clan-gated end-of-round `effect` meets an opposing effect in an
/// order no round pins, or an opposing Copy, which could take it or import an own write
/// (`life_writes` and `pillz_writes` report nothing for a Copy). Only the effects the gate
/// was admitted over are refused here; every combat-stat plan under `OwnerClanIn` answers
/// false.
/// - the opponent-Life reduction pays on its owner's win, so an opposing Life gain or an
///   order-sensitive own Life write landing on the opposing loss (or every round, for a
///   permanent) meets it;
/// - the Toxin latch pays every round after it latches, so any opposing own Life gain or
///   order-sensitive own Life write meets it, whatever outcome it lands on;
/// - the opponent-Pillz reduction meets an opposing own Pillz gain on the opposing loss;
/// - the Equalizer gains meet an opposing floor on the owner's resource on the opposing loss.
fn clan_gated_post_round_meets_unpinned_effect(
    effect: CombatStatEffectV1,
    opponent: &[CombatStatCardPlanV1; HAND_SIZE],
) -> bool {
    let on_opposing_loss = |plan: CombatStatSourcePlanV1| write_outcomes(plan).on_loss;
    let meets = match effect {
        CombatStatEffectV1::ReduceOpponentLifeOnVictory { .. } => {
            source_plans(opponent).any(|opposing| {
                let life = life_writes(opposing);
                on_opposing_loss(opposing) && (life.own_gain || life.own_order_sensitive)
            })
        }
        CombatStatEffectV1::ToxinOpponentLifeOnVictory { .. } => {
            source_plans(opponent).any(|opposing| {
                let life = life_writes(opposing);
                life.own_gain || life.own_order_sensitive
            })
        }
        CombatStatEffectV1::ReduceOpponentPillzOnVictory { .. } => source_plans(opponent)
            .any(|opposing| on_opposing_loss(opposing) && pillz_writes(opposing).own_gain),
        CombatStatEffectV1::GainLifeOnVictoryPerOpponentStars { .. } => source_plans(opponent)
            .any(|opposing| on_opposing_loss(opposing) && life_writes(opposing).opposing_floor),
        CombatStatEffectV1::GainPillzOnVictoryPerOpponentStars { .. } => source_plans(opponent)
            .any(|opposing| on_opposing_loss(opposing) && pillz_writes(opposing).opposing_floor),
        _ => return false,
    };
    meets || hand_has_copy(opponent)
}

/// Whether a revision-73 `Versus` or `After` end-of-round `plan` meets another write to the
/// resource it writes in an order no round pins (1093173/1 shows the server's cross-owner
/// order is not the engine's P1-then-P2), or an opposing Copy, which could take it or import
/// a writer (`life_writes` and `pillz_writes` report nothing for a Copy). Every combat-stat
/// plan under either gate - the numerics, `Stop Opp. Bonus` and the stat Copy - answers false.
/// - the opponent-Life reduction floors the opposing Life on its owner's win, so an opposing
///   own Life gain or order-sensitive own Life write landing on the opposing loss (or every
///   round, for a permanent) meets it;
/// - the uncapped Life gains commute with other uncapped gains, but not with an opposing
///   floor on the owner's Life landing on the opposing loss, nor with an order-sensitive own
///   Life write (a cap, a revival, a floor) from the card's other slot or an own latch;
/// - the Pillz gain likewise meets an opposing floor on the owner's Pillz on the opposing
///   loss, or an own capped Pillz gain from the other slot or an own latch.
///
/// Reading `write_outcomes` rather than every opposing floor keeps a Victory-only floor,
/// which can only land on the opposing win, from refusing a gain that pays on the owner's.
fn hand_clan_gated_post_round_meets_unpinned_effect(
    plan: CombatStatSourcePlanV1,
    effect: CombatStatEffectV1,
    own: &[CombatStatCardPlanV1; HAND_SIZE],
    opponent: &[CombatStatCardPlanV1; HAND_SIZE],
) -> bool {
    let on_opposing_loss = |plan: CombatStatSourcePlanV1| write_outcomes(plan).on_loss;
    let meets = match effect {
        CombatStatEffectV1::ReduceOpponentLifeOnVictory { .. } => {
            source_plans(opponent).any(|opposing| {
                let life = life_writes(opposing);
                on_opposing_loss(opposing) && (life.own_gain || life.own_order_sensitive)
            })
        }
        CombatStatEffectV1::GainLifeOnVictory { .. }
        | CombatStatEffectV1::GainLifePerFinalDamageOnVictory { .. } => {
            source_plans(opponent)
                .any(|opposing| on_opposing_loss(opposing) && life_writes(opposing).opposing_floor)
                || own_write_meets(plan, own, opponent, |own_plan| {
                    life_writes(own_plan).own_order_sensitive
                })
        }
        CombatStatEffectV1::GainPillzOnVictory { .. } => {
            source_plans(opponent)
                .any(|opposing| on_opposing_loss(opposing) && pillz_writes(opposing).opposing_floor)
                || own_write_meets(plan, own, opponent, |own_plan| {
                    pillz_writes(own_plan).own_capped
                })
        }
        _ => return false,
    };
    meets || hand_has_copy(opponent)
}

/// `same_owner_meets` with an own Copy judged by what it could import rather than by being a
/// Copy: the other slot of the card carrying `plan` answers `writes` itself, or is a Copy
/// whose slot kind holds an opposing plan that does; and any own latch answers `writes`, or
/// any own Copy could import an opposing latch that does, since an imported latch pays every
/// round after it latches. The adopted plan runs as the copier's own, so `writes` reads it
/// from the copier's side unchanged.
fn own_write_meets(
    plan: CombatStatSourcePlanV1,
    own: &[CombatStatCardPlanV1; HAND_SIZE],
    opponent: &[CombatStatCardPlanV1; HAND_SIZE],
    writes: impl Fn(CombatStatSourcePlanV1) -> bool,
) -> bool {
    let importable = |copied: CopiedSourceKindV1, latches_only: bool| {
        opponent.iter().any(|card| {
            let opposing = match copied {
                CopiedSourceKindV1::Ability => card.ability,
                CopiedSourceKindV1::Bonus => card.bonus,
            };
            (!latches_only || is_latch(opposing)) && writes(opposing)
        })
    };
    let other_slot = |other: CombatStatSourcePlanV1| match other {
        CombatStatSourcePlanV1::CopyOpponentSource { copied, .. } => importable(copied, false),
        _ => writes(other),
    };
    own.iter().any(|card| {
        (card.ability == plan && other_slot(card.bonus))
            || (card.bonus == plan && other_slot(card.ability))
    }) || source_plans(own).any(|own_plan| match own_plan {
        CombatStatSourcePlanV1::CopyOpponentSource { copied, .. } => importable(copied, true),
        _ => is_latch(own_plan) && writes(own_plan),
    })
}

/// Whether `plan` is a latch of the same family as the Unison `latch`: Poison or Toxin for a
/// Toxin, which the server's replacement note names together, and Consume for a Consume.
/// Combust, which also floors the opposing Pillz, is counted with Consume to stay on the
/// safe side of the open question.
fn same_latch_family(plan: CombatStatSourcePlanV1, latch: CombatStatEffectV1) -> bool {
    let CombatStatSourcePlanV1::Execute { effect, .. } = plan else {
        return false;
    };
    let Some(PostRoundSourceEffect::Fixed(
        PostRoundEffect::LatchOnVictory(latched)
        | PostRoundEffect::LatchOnDefeat(latched)
        | PostRoundEffect::LatchOnKillshot(latched),
    )) = shared_post_round_effect(effect)
    else {
        return false;
    };
    match latch {
        CombatStatEffectV1::ToxinOpponentLifeOnVictory { .. } => matches!(
            latched,
            LatchedEffectV1::PoisonOpponentLife { .. } | LatchedEffectV1::ToxinOpponentLife { .. }
        ),
        CombatStatEffectV1::ConsumeOpponentPillzOnVictory { .. } => matches!(
            latched,
            LatchedEffectV1::ConsumeOpponentPillz { .. }
                | LatchedEffectV1::CombustOpponentLifeAndPillz { .. }
        ),
        _ => false,
    }
}

/// Whether an opposing floor on a Unison gain's Life can land in a round the gain pays. A latch
/// pays every round after it latches, so it always can. Otherwise both the floor's slot
/// predicate and its outcome must allow the round: `Symmetry:` fires only when the two
/// selected cards share a slot and `Asymmetry:` only when they differ, and the floor must
/// write on the outcome the gain pays on - a Defeat gain on its owner's loss, the opposing
/// win, and the Victory compound on the opposing loss. `same_slot` says whether the floor's
/// card sits in the gain's slot. Everything else answers true, so this only ever drops a
/// floor that provably cannot coincide (1023495: Doela Noel's `Symmetry:` reduction in slot 0
/// against Pantherine in slot 3).
fn opposing_floor_can_meet_unison_gain(
    opposing: CombatStatSourcePlanV1,
    same_slot: bool,
    gain: CombatStatEffectV1,
) -> bool {
    if is_latch(opposing) {
        return true;
    }
    let CombatStatSourcePlanV1::Execute { predicate, .. } = opposing else {
        return true;
    };
    let slot_allows = match predicate {
        CombatStatPredicateV1::SelectedHandSlotsMatch => same_slot,
        CombatStatPredicateV1::SelectedHandSlotsDiffer => !same_slot,
        _ => true,
    };
    let outcomes = write_outcomes(opposing);
    let outcome_allows = match gain {
        CombatStatEffectV1::GainLifeOnDefeat { .. } => outcomes.on_win,
        _ => outcomes.on_loss,
    };
    slot_allows && outcome_allows
}

/// True when the Life a `/ Life Lost` source reads could rise during the match. Its owner
/// reads it, and so does an opposing Copy that adopts it, so the Life of each player who
/// could hold it must be one that can only fall.
fn life_lost_reader_life_can_rise(
    own: &[CombatStatCardPlanV1; HAND_SIZE],
    opponent: &[CombatStatCardPlanV1; HAND_SIZE],
) -> bool {
    player_life_can_rise(own, opponent)
        || (hand_has_copy(opponent) && player_life_can_rise(opponent, own))
}

/// True when anything could raise this player's Life: one of their own executable
/// post-round effects that gains Life for its owner or for both players, an opposing one
/// that gains Life for both players, or either kind reaching them through a Copy - their own
/// Copy adopting an opposing gain, or an opposing Copy adopting their own both-players gain.
fn player_life_can_rise(
    hand: &[CombatStatCardPlanV1; HAND_SIZE],
    opposing: &[CombatStatCardPlanV1; HAND_SIZE],
) -> bool {
    let beneficiary = |plan: CombatStatSourcePlanV1| match plan {
        CombatStatSourcePlanV1::Execute { effect, .. } => {
            shared_post_round_effect(effect).map(PostRoundSourceEffect::life_beneficiary)
        }
        CombatStatSourcePlanV1::CopyOpponentSource { .. }
        | CombatStatSourcePlanV1::Absent
        | CombatStatSourcePlanV1::Disabled { .. }
        | CombatStatSourcePlanV1::RejectIfSelected { .. } => None,
    };
    let any_gain = |hand: &[CombatStatCardPlanV1; HAND_SIZE]| {
        source_plans(hand).any(|plan| {
            matches!(
                beneficiary(plan),
                Some(LifeBeneficiaryV1::Owner | LifeBeneficiaryV1::Both)
            )
        })
    };
    let both_gain = |hand: &[CombatStatCardPlanV1; HAND_SIZE]| {
        source_plans(hand).any(|plan| beneficiary(plan) == Some(LifeBeneficiaryV1::Both))
    };
    any_gain(hand)
        || both_gain(opposing)
        || (hand_has_copy(hand) && any_gain(opposing))
        || (hand_has_copy(opposing) && both_gain(hand))
}

fn source_plans(
    hand: &[CombatStatCardPlanV1; HAND_SIZE],
) -> impl Iterator<Item = CombatStatSourcePlanV1> + '_ {
    hand.iter().flat_map(|card| [card.ability, card.bonus])
}

/// Whether an own effect that can land in the same round as `plan` answers `writes`: the
/// other slot of a card carrying `plan`, which fires beside it, or a latched permanent from
/// anywhere in the owner's hand, which pays every round after it latches. Another own card's
/// fresh effects cannot share `plan`'s round. A Copy in the other slot could import any
/// opposing writer, so it answers true.
fn same_owner_meets(
    plan: CombatStatSourcePlanV1,
    own: &[CombatStatCardPlanV1; HAND_SIZE],
    writes: impl Fn(CombatStatSourcePlanV1) -> bool,
) -> bool {
    let other_slot = |other: CombatStatSourcePlanV1| {
        matches!(other, CombatStatSourcePlanV1::CopyOpponentSource { .. }) || writes(other)
    };
    own.iter().any(|card| {
        (card.ability == plan && other_slot(card.bonus))
            || (card.bonus == plan && other_slot(card.ability))
    }) || source_plans(own).any(|own_plan| is_latch(own_plan) && writes(own_plan))
}

/// Whether an end-of-round `plan` writes its own owner's Life in any way.
fn writes_own_life(plan: CombatStatSourcePlanV1) -> bool {
    let life = life_writes(plan);
    life.own_gain || life.own_order_sensitive
}

/// A permanent: its owner latches it and it pays every later round.
fn is_latch(plan: CombatStatSourcePlanV1) -> bool {
    let CombatStatSourcePlanV1::Execute { effect, .. } = plan else {
        return false;
    };
    matches!(
        shared_post_round_effect(effect),
        Some(PostRoundSourceEffect::Fixed(
            PostRoundEffect::LatchOnVictory(_)
                | PostRoundEffect::LatchOnDefeat(_)
                | PostRoundEffect::LatchOnKillshot(_)
        ))
    )
}

/// Whose Life an end-of-round `plan` can raise, if it is end-of-round work at all.
fn life_beneficiary(plan: CombatStatSourcePlanV1) -> Option<LifeBeneficiaryV1> {
    let CombatStatSourcePlanV1::Execute { effect, .. } = plan else {
        return None;
    };
    shared_post_round_effect(effect).map(PostRoundSourceEffect::life_beneficiary)
}

/// Whether an opposing Copy could take `plan`: one copying the slot, Ability or Bonus, an
/// owner card carries it in.
fn opposing_copy_can_take(
    plan: CombatStatSourcePlanV1,
    own: &[CombatStatCardPlanV1; HAND_SIZE],
    opponent: &[CombatStatCardPlanV1; HAND_SIZE],
) -> bool {
    let copies = |kind| {
        source_plans(opponent).any(|opposing| {
            matches!(
                opposing,
                CombatStatSourcePlanV1::CopyOpponentSource { copied, .. } if copied == kind
            )
        })
    };
    own.iter().any(|card| {
        (card.ability == plan && copies(CopiedSourceKindV1::Ability))
            || (card.bonus == plan && copies(CopiedSourceKindV1::Bonus))
    })
}

/// Whether an opposing Bonus-slot Copy could take `plan` and run it from its Bonus slot.
fn opposing_bonus_copy_can_take(
    plan: CombatStatSourcePlanV1,
    own: &[CombatStatCardPlanV1; HAND_SIZE],
    opponent: &[CombatStatCardPlanV1; HAND_SIZE],
) -> bool {
    let copies = |kind| {
        opponent.iter().any(|opposing| {
            matches!(
                opposing.bonus,
                CombatStatSourcePlanV1::CopyOpponentSource { copied, .. } if copied == kind
            )
        })
    };
    own.iter().any(|card| {
        (card.ability == plan && copies(CopiedSourceKindV1::Ability))
            || (card.bonus == plan && copies(CopiedSourceKindV1::Bonus))
    })
}

fn hand_has_copy(hand: &[CombatStatCardPlanV1; HAND_SIZE]) -> bool {
    source_plans(hand).any(|plan| matches!(plan, CombatStatSourcePlanV1::CopyOpponentSource { .. }))
}

/// A Killshot reads both final Attacks. `PostRoundEffect::reads_final_attacks` is
/// exhaustive, so a later Killshot grammar cannot slip past the `Tune Out` refusal.
fn is_killshot(effect: CombatStatEffectV1) -> bool {
    shared_post_round_effect(effect).is_some_and(PostRoundSourceEffect::reads_final_attacks)
}

fn simplifies_attack(plan: CombatStatSourcePlanV1) -> bool {
    matches!(
        plan,
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::SimplifyAttackToPillz,
            ..
        }
    )
}

/// Whether an opposing end-of-round effect writes a resource `permanent` floors: Pillz for
/// `Consume`, Life or Pillz for `Combust`. A permanent of the opposing player counts too,
/// whatever it writes, since its own repeat meets this one every round.
fn writes_floored_resource(plan: CombatStatSourcePlanV1, permanent: CombatStatEffectV1) -> bool {
    let CombatStatSourcePlanV1::Execute { effect, .. } = plan else {
        return false;
    };
    let Some(effect) = shared_post_round_effect(effect) else {
        return false;
    };
    let floors_life = matches!(
        permanent,
        CombatStatEffectV1::CombustOpponentLifeAndPillzOnVictory { .. }
    );
    match effect.resource() {
        PostRoundResourceV1::Pillz
        | PostRoundResourceV1::PillzAndLife
        | PostRoundResourceV1::BothPlayersPillz
        | PostRoundResourceV1::Permanent => true,
        PostRoundResourceV1::Life | PostRoundResourceV1::BothPlayersLife => floors_life,
    }
}

/// A reduction that can take the opposing card's Attack to 0: a Min 0 cut of its Attack, or
/// of its Power, on either the opposing side alone or both.
fn zeroes_opposing_attack(plan: CombatStatSourcePlanV1) -> bool {
    matches!(
        plan,
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::ModifyCombatStat {
                side: CombatStatAffectedSideV1::Opponent | CombatStatAffectedSideV1::Both,
                stat: CombatStatAttributeV1::Attack
                    | CombatStatAttributeV1::Power
                    | CombatStatAttributeV1::PowerAndDamage,
                operation: CombatStatOperationV1::Decrease,
                minimum: Some(0),
                ..
            },
            ..
        }
    )
}

/// A `Cards` reduction to Min 0, which can take both Attacks to 0 on its own.
fn zeroes_both_attacks(plan: CombatStatSourcePlanV1) -> bool {
    matches!(
        plan,
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::ModifyCombatStat {
                side: CombatStatAffectedSideV1::Both,
                stat: CombatStatAttributeV1::Attack
                    | CombatStatAttributeV1::Power
                    | CombatStatAttributeV1::PowerAndDamage,
                operation: CombatStatOperationV1::Decrease,
                minimum: Some(0),
                ..
            },
            ..
        }
    )
}

/// The one Recover revision 8 admitted, whose contexts stay as they were.
const REVISION_8_RECOVERY: CombatStatEffectV1 = CombatStatEffectV1::RecoverPaidPillzOnDefeat {
    numerator: 2,
    denominator: 3,
};

/// Whose Pillz an end-of-round `plan` writes, from its own owner's side.
/// `PostRoundSourceEffect::pillz_writes` is exhaustive, so a later grammar cannot slip past
/// the Recover and Dope refusals.
fn pillz_writes(plan: CombatStatSourcePlanV1) -> PillzWritesV1 {
    let CombatStatSourcePlanV1::Execute { effect, .. } = plan else {
        return PillzWritesV1::NONE;
    };
    match shared_post_round_effect(effect) {
        Some(effect) => effect.pillz_writes(),
        None => PillzWritesV1::NONE,
    }
}

/// Whose Life an end-of-round `plan` writes; `life_writes` is exhaustive like `pillz_writes`.
fn life_writes(plan: CombatStatSourcePlanV1) -> LifeWritesV1 {
    let CombatStatSourcePlanV1::Execute { effect, .. } = plan else {
        return LifeWritesV1::NONE;
    };
    match shared_post_round_effect(effect) {
        Some(effect) => effect.life_writes(),
        None => LifeWritesV1::NONE,
    }
}

/// On which of its owner's outcomes an end-of-round `plan` writes; nothing for a plan that
/// is not end-of-round work.
fn write_outcomes(plan: CombatStatSourcePlanV1) -> WriteOutcomesV1 {
    let never = WriteOutcomesV1 {
        on_win: false,
        on_loss: false,
    };
    let CombatStatSourcePlanV1::Execute { effect, .. } = plan else {
        return never;
    };
    match shared_post_round_effect(effect) {
        Some(effect) => effect.write_outcomes(),
        None => never,
    }
}

fn floors_opposing_pillz(plan: CombatStatSourcePlanV1) -> bool {
    pillz_writes(plan).opposing_floor
}

/// Whether `plan`, from the opposing hand, changes or reads a stat of the owner's card that no
/// captured round has shown meeting a single-stat `protected` Protection: Attack for
/// `Protection: Attack`, and Power or Damage for the other two. `Tune Out` changes both.
fn meets_unpinned_protection(
    plan: CombatStatSourcePlanV1,
    protected: CombatStatAttributeV1,
) -> bool {
    let CombatStatSourcePlanV1::Execute { effect, .. } = plan else {
        return false;
    };
    let unpinned = |stat| {
        (stat == CombatStatAttributeV1::Attack) == (protected == CombatStatAttributeV1::Attack)
    };
    match effect {
        CombatStatEffectV1::ModifyCombatStat {
            side: CombatStatAffectedSideV1::Opponent | CombatStatAffectedSideV1::Both,
            stat,
            ..
        }
        | CombatStatEffectV1::CancelOpponentCombatStatModifiers { stat }
        | CombatStatEffectV1::CopyOpponentPrintedCombatStat { stat }
        | CombatStatEffectV1::ExchangePrintedCombatStat { stat }
        | CombatStatEffectV1::ImposePrintedCombatStat { stat } => unpinned(stat),
        CombatStatEffectV1::SimplifyAttackToPillz => true,
        _ => false,
    }
}

/// An opposing Cancel of Damage modifiers, alone or with Power.
fn cancels_damage_modifiers(plan: CombatStatSourcePlanV1) -> bool {
    matches!(
        plan,
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::CancelOpponentCombatStatModifiers {
                stat: CombatStatAttributeV1::Damage | CombatStatAttributeV1::PowerAndDamage,
            },
            ..
        }
    )
}

/// A Power reduction whose floor is 0, from either side's opposing phase.
fn reduces_power_to_zero(plan: CombatStatSourcePlanV1) -> bool {
    matches!(
        plan,
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::ModifyCombatStat {
                side: CombatStatAffectedSideV1::Opponent | CombatStatAffectedSideV1::Both,
                stat: CombatStatAttributeV1::Power | CombatStatAttributeV1::PowerAndDamage,
                operation: CombatStatOperationV1::Decrease,
                minimum: Some(0),
                ..
            },
            ..
        }
    )
}

/// Whether an opposing source cancels the owner's modifiers of `stat`.
fn cancels_modifiers_of(plan: CombatStatSourcePlanV1, stat: CombatStatAttributeV1) -> bool {
    let CombatStatSourcePlanV1::Execute {
        effect: CombatStatEffectV1::CancelOpponentCombatStatModifiers { stat: cancelled },
        ..
    } = plan
    else {
        return false;
    };
    cancelled == stat
        || (cancelled == CombatStatAttributeV1::PowerAndDamage
            && matches!(
                stat,
                CombatStatAttributeV1::Power | CombatStatAttributeV1::Damage
            ))
}

/// Whether an opposing source reads, or could undo, the Attacks `Tune Out` replaces: a
/// Killshot, or a cancel of Power or Attack modifiers.
fn reads_simplified_attack(plan: CombatStatSourcePlanV1) -> bool {
    let CombatStatSourcePlanV1::Execute { effect, .. } = plan else {
        return false;
    };
    is_killshot(effect)
        || matches!(
            effect,
            CombatStatEffectV1::CancelOpponentCombatStatModifiers {
                stat: CombatStatAttributeV1::Attack
                    | CombatStatAttributeV1::Power
                    | CombatStatAttributeV1::PowerAndDamage,
            }
        )
}

fn opponent_defeats_resource_cancellation(
    resources: crate::effect_registry::ResourceCancellationV1,
    opponent: &[CombatStatCardPlanV1; HAND_SIZE],
) -> bool {
    opponent.iter().any(|card| {
        [card.ability, card.bonus]
            .into_iter()
            .any(|plan| match plan {
                CombatStatSourcePlanV1::CopyOpponentSource { .. } => true,
                CombatStatSourcePlanV1::Execute { effect, .. } => shared_post_round_effect(effect)
                    .is_some_and(|effect| match effect.resource() {
                        PostRoundResourceV1::Life => false,
                        PostRoundResourceV1::Pillz => {
                            resources
                                == crate::effect_registry::ResourceCancellationV1::PillzAndLife
                        }
                        PostRoundResourceV1::PillzAndLife
                        | PostRoundResourceV1::BothPlayersLife
                        | PostRoundResourceV1::BothPlayersPillz
                        | PostRoundResourceV1::Permanent => true,
                    }),
                CombatStatSourcePlanV1::Absent
                | CombatStatSourcePlanV1::Disabled { .. }
                | CombatStatSourcePlanV1::RejectIfSelected { .. } => false,
            })
    })
}

/// `After` and `Versus` read canonical clans, as the printed rules text says ("the Oculus,
/// even when infiltrated ..., do not activate this condition"). No captured round separates
/// that from the effective clan, so a match where the two readings would disagree - an
/// infiltrating Oculus in the hand the gate reads whose effective clan and Oculus itself fall
/// on opposite sides of the list - is refused rather than executed on an unpinned rule.
fn validate_clan_gate_context(
    player: PlayerId,
    hand_slot: HandSlot,
    spec: &CombatStatDiagnosticMatchSpecV1,
) -> Result<(), CombatStatPlanErrorV1> {
    let card = spec.cards[player][hand_slot.index()];
    for (source, plan) in [
        (CombatStatEffectSourceV1::Ability, card.ability),
        (CombatStatEffectSourceV1::Bonus, card.bonus),
    ] {
        let CombatStatSourcePlanV1::Execute {
            source_id,
            predicate,
            ..
        } = plan
        else {
            continue;
        };
        if clan_gate_is_ambiguous(player, source, predicate, &spec.base_rules, &spec.cards) {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::AmbiguousOculusClanGate,
            ));
        }
    }
    Ok(())
}

/// Whether a `Versus` or `After` gate on a plan in `owner`'s `source` slot could read an
/// infiltrating Oculus whose canonical and effective clans fall on opposite sides of the
/// gate's list. Since revision 73 it looks only at the hand the gate reads: `Versus` reads
/// the opposing hand and `After` the owner's own previous card, so an Oculus elsewhere cannot
/// change either. An opposing Copy of the slot kind carrying the plan widens it to both
/// hands, because an adopted plan keeps its predicate but is judged from the copier's seat -
/// a copied `Versus` reads the original owner's hand, a copied `After` the copier's previous
/// card. Every other predicate answers false.
pub fn clan_gate_is_ambiguous(
    owner: PlayerId,
    source: CombatStatEffectSourceV1,
    predicate: CombatStatPredicateV1,
    base_rules: &BaseRulesMatchSpec,
    cards: &ByPlayer<[CombatStatCardPlanV1; HAND_SIZE]>,
) -> bool {
    const OCULUS: u32 = 56;
    let (set, read) = match predicate {
        CombatStatPredicateV1::OpponentHandHasClan(set) => (set, owner.other()),
        CombatStatPredicateV1::OwnerPreviousCardClanIn(set) => (set, owner),
        _ => return false,
    };
    let copied = match source {
        CombatStatEffectSourceV1::Ability => CopiedSourceKindV1::Ability,
        CombatStatEffectSourceV1::Bonus => CopiedSourceKindV1::Bonus,
    };
    let adoptable = source_plans(&cards[owner.other()]).any(|plan| {
        matches!(
            plan,
            CombatStatSourcePlanV1::CopyOpponentSource { copied: kind, .. } if kind == copied
        )
    });
    let infiltrated = |side: PlayerId| {
        (0..HAND_SIZE).any(|index| {
            let canonical = base_rules.players[side].hand[index].clan_id;
            let effective = cards[side][index].effective_clan_id;
            canonical == OCULUS
                && effective != OCULUS
                && set.contains(OCULUS) != set.contains(effective)
        })
    };
    infiltrated(read) || (adoptable && infiltrated(read.other()))
}

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

/// `1730` is `Growth: - 1 Opp. Life Min 4`, a round-scaled magnitude rather than a
/// predicate, so it may not ride the plain fixed grammar however a caller labels it. Since
/// revision 53 it executes on its own round-scaled variant instead.
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
    if let CombatStatSourcePlanV1::CopyOpponentSource {
        source_id,
        predicate,
        ..
    } = plan
    {
        return if source_id == 0 {
            Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::CopyOpponentSourceIdentity,
            ))
        } else if source == CombatStatEffectSourceV1::Bonus
            && matches!(
                predicate,
                CombatStatPredicateV1::OwnerClanIn(_) | CombatStatPredicateV1::OwnerClanInAnd(..)
            )
        {
            // The clan-gated Copies (revision 70) are printed abilities; no clan bonus
            // prints a clan gate over a Copy.
            Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::ConditionalBonus,
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
    // The Victory Or Defeat own gains beyond the reviewed identities are card abilities only
    // and unconditional; the Pillz amount starts at two because one is the reviewed set.
    if let CombatStatEffectV1::GainPillzOnVictoryOrDefeat { pillz: amount }
    | CombatStatEffectV1::GainLifePerFinalDamageOnVictoryOrDefeat {
        life_per_damage: amount,
    } = effect
    {
        let minimum = if matches!(
            effect,
            CombatStatEffectV1::GainPillzOnVictoryOrDefeat { .. }
        ) {
            2
        } else {
            1
        };
        let reason = if source != CombatStatEffectSourceV1::Ability {
            Some(InvalidCombatStatPlanReasonV1::VictoryOrDefeatGainSource)
        } else if amount < minimum {
            Some(InvalidCombatStatPlanReasonV1::VictoryOrDefeatGainMagnitude)
        } else if predicate != CombatStatPredicateV1::Always {
            Some(InvalidCombatStatPlanReasonV1::VictoryOrDefeatGainPredicate)
        } else {
            None
        };
        return match reason {
            Some(reason) => Err(invalid_combat_stat_execute(
                player, hand_slot, source, source_id, reason,
            )),
            None => Ok(()),
        };
    }
    // The opposing compound is a card ability only, positive and unconditional.
    if let CombatStatEffectV1::ReduceOpponentPillzAndLifeOnVictory { amount, .. } = effect {
        let reason = if source != CombatStatEffectSourceV1::Ability {
            Some(InvalidCombatStatPlanReasonV1::VictoryOpponentPillzAndLifeSource)
        } else if amount == 0 {
            Some(InvalidCombatStatPlanReasonV1::VictoryOpponentPillzAndLifeMagnitude)
        } else if predicate != CombatStatPredicateV1::Always {
            Some(InvalidCombatStatPlanReasonV1::VictoryOpponentPillzAndLifePredicate)
        } else {
            None
        };
        return match reason {
            Some(reason) => Err(invalid_combat_stat_execute(
                player, hand_slot, source, source_id, reason,
            )),
            None => Ok(()),
        };
    }
    // The Unison Victory compound: a card ability, positive, and only under its gate - the
    // unconditional `+1 Pillz And Life` is Komboka's identity-locked bonus above.
    if let CombatStatEffectV1::GainPillzAndLifeOnVictory { amount } = effect {
        let reason = if source != CombatStatEffectSourceV1::Ability {
            Some(InvalidCombatStatPlanReasonV1::UnisonPillzAndLifeSource)
        } else if amount == 0 {
            Some(InvalidCombatStatPlanReasonV1::UnisonPillzAndLifeMagnitude)
        } else if predicate != CombatStatPredicateV1::OwnerHandUnison {
            Some(InvalidCombatStatPlanReasonV1::UnisonPillzAndLifePredicate)
        } else {
            None
        };
        return match reason {
            Some(reason) => Err(invalid_combat_stat_execute(
                player, hand_slot, source, source_id, reason,
            )),
            None => Ok(()),
        };
    }
    match effect {
        // Dope is Regen on the owner's Pillz, from either outcome channel, with no prefix.
        CombatStatEffectV1::DopePillzOnVictory { pillz, maximum }
        | CombatStatEffectV1::DopePillzOnDefeat { pillz, maximum } => {
            let reason = if source != CombatStatEffectSourceV1::Ability {
                Some(InvalidCombatStatPlanReasonV1::PermanentLifeSource)
            } else if pillz == 0 || maximum <= pillz {
                Some(InvalidCombatStatPlanReasonV1::PermanentLifeMagnitude)
            } else if predicate != CombatStatPredicateV1::Always {
                Some(InvalidCombatStatPlanReasonV1::PermanentLifePredicate)
            } else {
                None
            };
            return match reason {
                Some(reason) => Err(invalid_combat_stat_execute(
                    player, hand_slot, source, source_id, reason,
                )),
                None => Ok(()),
            };
        }
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
        | CombatStatEffectV1::PoisonOpponentLifeOnDefeat { life, .. }
        | CombatStatEffectV1::ToxinOpponentLifeOnVictory { life, .. }
        | CombatStatEffectV1::ConsumeOpponentPillzOnVictory { pillz: life, .. }
        | CombatStatEffectV1::CombustOpponentLifeAndPillzOnVictory { amount: life, .. } => {
            // Revision 70 puts the owner-clan gate on Toxin (`5613`) and the gate with
            // Reprisal's second move on Consume (`5275`); each is its own grammar's only
            // gated form, and both are card abilities.
            // Revision 73 puts the one-clan hand gate on both (`5316`, `4695`).
            let clan_gated = matches!(
                (effect, predicate),
                (
                    CombatStatEffectV1::ToxinOpponentLifeOnVictory { .. },
                    CombatStatPredicateV1::OwnerClanIn(_) | CombatStatPredicateV1::OwnerHandUnison,
                ) | (
                    CombatStatEffectV1::ConsumeOpponentPillzOnVictory { .. },
                    CombatStatPredicateV1::OwnerClanInAnd(_, ClanConjunctV1::OwnerMovesSecond)
                        | CombatStatPredicateV1::OwnerHandUnison,
                )
            );
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
            if !permanent_predicate_admitted(predicate) && !clan_gated {
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
        // `Night: -N Opp. Life Min M` (Lyra's `4750`) puts the match constant on it since
        // revision 69, Phalloide Ld's `[clan:..] - 2 Opp. Life Min 2` (`5392`) the
        // owner-clan gate since revision 70, and the `Versus` and `After` gates (`5505`,
        // `5283`, `5602`) since revision 73.
        if !matches!(
            predicate,
            CombatStatPredicateV1::Always
                | CombatStatPredicateV1::OwnerPillzUsedAbove(_)
                | CombatStatPredicateV1::MatchIsNight
                | CombatStatPredicateV1::OwnerClanIn(_)
                | CombatStatPredicateV1::OpponentHandHasClan(_)
                | CombatStatPredicateV1::OwnerPreviousCardClanIn(_)
        ) {
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
    // Every other Equalizer post-round effect is a grammar: the opponent-Life reduction at
    // any printed numbers and the two own gains. Card abilities only - a captured Copy's
    // Bonus provenance is the reviewed identities' alone - with a positive amount per star
    // and no condition beyond the outcome the engine resolves, except that since revision
    // 70 the two own gains may carry the owner-clan gate (`5165`, `5616`).
    if let CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars { per_star, .. }
    | CombatStatEffectV1::GainLifeOnVictoryPerOpponentStars { per_star }
    | CombatStatEffectV1::GainPillzOnVictoryPerOpponentStars { per_star } = effect
    {
        let clan_gated_gain = matches!(predicate, CombatStatPredicateV1::OwnerClanIn(_))
            && matches!(
                effect,
                CombatStatEffectV1::GainLifeOnVictoryPerOpponentStars { .. }
                    | CombatStatEffectV1::GainPillzOnVictoryPerOpponentStars { .. }
            );
        let reason = if source != CombatStatEffectSourceV1::Ability {
            Some(InvalidCombatStatPlanReasonV1::EqualizerPostRoundSource)
        } else if per_star == 0 {
            Some(InvalidCombatStatPlanReasonV1::EqualizerPostRoundMagnitude)
        } else if predicate != CombatStatPredicateV1::Always && !clan_gated_gain {
            Some(InvalidCombatStatPlanReasonV1::EqualizerPostRoundPredicate)
        } else {
            None
        };
        return match reason {
            Some(reason) => Err(invalid_combat_stat_execute(
                player, hand_slot, source, source_id, reason,
            )),
            None => Ok(()),
        };
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
    // Recovery is admitted by grammar since revision 63. A zero denominator would divide
    // by zero in the engine arm, and no card prints a ratio of one or more.
    if let CombatStatEffectV1::RecoverPaidPillzOnDefeat {
        numerator,
        denominator,
    }
    | CombatStatEffectV1::RecoverPaidPillzOnVictory {
        numerator,
        denominator,
    } = effect
    {
        if numerator == 0 || numerator >= denominator {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::RecoveryRatio,
            ));
        }
        let on_victory = matches!(effect, CombatStatEffectV1::RecoverPaidPillzOnVictory { .. });
        // Only the Vortex bonus prints a Recover, and it prints the Defeat form.
        if on_victory && source != CombatStatEffectSourceV1::Ability {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::RecoverySource,
            ));
        }
        // `Unison :` prints only the Victory form.
        if !(predicate == CombatStatPredicateV1::Always
            || (on_victory && predicate == CombatStatPredicateV1::OwnerHandUnison))
        {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::RecoveryPredicate,
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
        // The plain grammar is unconditional; the three reviewed prefixed forms carry one
        // already-resolved predicate each, and all are card abilities only - as are the
        // revision-73 `Versus` and `After` gates (`3545`; `5670`, `5701`, `5723`).
        // `Bet > N Pillz:` is the one gate a clan bonus prints on it (the Zenith bonus).
        let bet_gate = matches!(predicate, CombatStatPredicateV1::OwnerPillzUsedAbove(_));
        if !(bet_gate
            || matches!(
                predicate,
                CombatStatPredicateV1::Always
                    | CombatStatPredicateV1::OwnerWonPreviousRound
                    | CombatStatPredicateV1::SelectedHandSlotsDiffer
                    | CombatStatPredicateV1::OwnerMovesFirst
                    | CombatStatPredicateV1::OpponentHandHasClan(_)
                    | CombatStatPredicateV1::OwnerPreviousCardClanIn(_)
            ))
            || (predicate != CombatStatPredicateV1::Always
                && !bet_gate
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
        // The plain grammar is unconditional; the two reviewed prefixed forms carry the
        // previous-round predicate `Confidence:` names or the first move `Courage:` names,
        // and revision 73's `After` gate (`5700`) the previous card's clan. All are card
        // abilities only.
        if !matches!(
            predicate,
            CombatStatPredicateV1::Always
                | CombatStatPredicateV1::OwnerWonPreviousRound
                | CombatStatPredicateV1::OwnerMovesFirst
                | CombatStatPredicateV1::OwnerPreviousCardClanIn(_)
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
    // The capped Victory Pillz grammar: a card ability, a positive gain under a positive cap,
    // unconditional or under the `Night:` match constant its night form prints.
    if let CombatStatEffectV1::GainPillzOnVictoryMax { pillz, maximum } = effect {
        let reason = if source != CombatStatEffectSourceV1::Ability {
            Some(InvalidCombatStatPlanReasonV1::VictoryPillzMaxSource)
        } else if pillz == 0 || maximum == 0 {
            Some(InvalidCombatStatPlanReasonV1::VictoryPillzMaxMagnitude)
        } else if !matches!(
            predicate,
            CombatStatPredicateV1::Always | CombatStatPredicateV1::MatchIsNight
        ) {
            Some(InvalidCombatStatPlanReasonV1::VictoryPillzMaxPredicate)
        } else {
            None
        };
        return match reason {
            Some(reason) => Err(invalid_combat_stat_execute(
                player, hand_slot, source, source_id, reason,
            )),
            None => Ok(()),
        };
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
        // Dark Kaizerin's `[clan:..] -2 Opp Pillz. Min 2` (`4037`, `4038`) puts the
        // owner-clan gate on it since revision 70.
        if !matches!(
            predicate,
            CombatStatPredicateV1::Always
                | CombatStatPredicateV1::OwnerPillzUsedAbove(_)
                | CombatStatPredicateV1::OwnerClanIn(_)
        ) {
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
    // The round-scaled Victory grammars are card abilities only, positive and unconditional.
    if let CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerRound { per_round, .. }
    | CombatStatEffectV1::ReduceOpponentPillzOnVictoryPerRound { per_round, .. }
    | CombatStatEffectV1::GainLifeOnVictoryPerRound { per_round, .. }
    | CombatStatEffectV1::GainPillzOnVictoryPerRound { per_round, .. } = effect
    {
        let reason = if source != CombatStatEffectSourceV1::Ability {
            Some(InvalidCombatStatPlanReasonV1::RoundScaledPostRoundSource)
        } else if per_round == 0 {
            Some(InvalidCombatStatPlanReasonV1::RoundScaledPostRoundMagnitude)
        } else if predicate != CombatStatPredicateV1::Always {
            Some(InvalidCombatStatPlanReasonV1::RoundScaledPostRoundPredicate)
        } else {
            None
        };
        return match reason {
            Some(reason) => Err(invalid_combat_stat_execute(
                player, hand_slot, source, source_id, reason,
            )),
            None => Ok(()),
        };
    }
    // The both-players Victory Or Defeat Pillz gain is a card ability only, positive and
    // unconditional. The Life form is refused outright: no round shows it meeting a
    // knockout, which the Pillz form shows the server pays through.
    if let CombatStatEffectV1::GainBothPlayersLifeOnVictoryOrDefeat { life: amount }
    | CombatStatEffectV1::GainBothPlayersPillzOnVictoryOrDefeat { pillz: amount } = effect
    {
        let reason = if matches!(
            effect,
            CombatStatEffectV1::GainBothPlayersLifeOnVictoryOrDefeat { .. }
        ) {
            Some(InvalidCombatStatPlanReasonV1::BothPlayersLifeGainAgainstKnockout)
        } else if source != CombatStatEffectSourceV1::Ability {
            Some(InvalidCombatStatPlanReasonV1::BothPlayersGainSource)
        } else if amount == 0 {
            Some(InvalidCombatStatPlanReasonV1::BothPlayersGainMagnitude)
        } else if predicate != CombatStatPredicateV1::Always {
            Some(InvalidCombatStatPlanReasonV1::BothPlayersGainPredicate)
        } else {
            None
        };
        return match reason {
            Some(reason) => Err(invalid_combat_stat_execute(
                player, hand_slot, source, source_id, reason,
            )),
            None => Ok(()),
        };
    }
    // The Defeat own Pillz gain and its compound are card abilities only, positive and
    // unconditional.
    if let CombatStatEffectV1::GainPillzOnDefeat { pillz: amount }
    | CombatStatEffectV1::GainPillzAndLifeOnDefeat { amount } = effect
    {
        let reason = if source != CombatStatEffectSourceV1::Ability {
            Some(InvalidCombatStatPlanReasonV1::DefeatPillzSource)
        } else if amount == 0 {
            Some(InvalidCombatStatPlanReasonV1::DefeatPillzMagnitude)
        } else if predicate != CombatStatPredicateV1::Always {
            Some(InvalidCombatStatPlanReasonV1::DefeatPillzPredicate)
        } else {
            None
        };
        return match reason {
            Some(reason) => Err(invalid_combat_stat_execute(
                player, hand_slot, source, source_id, reason,
            )),
            None => Ok(()),
        };
    }
    // The Killshot compound and the resource cancellers are card abilities only; the
    // compound needs a positive amount and no predicate of its own.
    if let CombatStatEffectV1::GainPillzAndLifeOnKillshot { amount } = effect {
        if source != CombatStatEffectSourceV1::Ability {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::KillshotPillzAndLifeSource,
            ));
        }
        if amount == 0 {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::KillshotPillzAndLifeMagnitude,
            ));
        }
        if predicate != CombatStatPredicateV1::Always {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::KillshotPillzAndLifePredicate,
            ));
        }
        return Ok(());
    }
    // Its halves on their own and the ratio-latched Toxin are card abilities only with a
    // positive magnitude. The one predicate any of them prints is `Unison:`, on the
    // uncapped Life gain; a capped or Pillz form under a gate has never been printed.
    if let CombatStatEffectV1::GainPillzOnKillshot { pillz: amount }
    | CombatStatEffectV1::GainLifeOnKillshot { life: amount, .. }
    | CombatStatEffectV1::ToxinOpponentLifeOnKillshot { life: amount, .. } = effect
    {
        let unison_life = matches!(
            effect,
            CombatStatEffectV1::GainLifeOnKillshot { maximum: 0, .. }
        ) && predicate == CombatStatPredicateV1::OwnerHandUnison;
        let reason = if source != CombatStatEffectSourceV1::Ability {
            Some(InvalidCombatStatPlanReasonV1::KillshotPostRoundSource)
        } else if amount == 0 {
            Some(InvalidCombatStatPlanReasonV1::KillshotPostRoundMagnitude)
        } else if predicate != CombatStatPredicateV1::Always && !unison_life {
            Some(InvalidCombatStatPlanReasonV1::KillshotPostRoundPredicate)
        } else {
            None
        };
        return match reason {
            Some(reason) => Err(invalid_combat_stat_execute(
                player, hand_slot, source, source_id, reason,
            )),
            None => Ok(()),
        };
    }
    if matches!(
        effect,
        CombatStatEffectV1::CancelOpponentResourceModifiers { .. }
    ) && source != CombatStatEffectSourceV1::Ability
    {
        return Err(invalid_combat_stat_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidCombatStatPlanReasonV1::ResourceCancellationSource,
        ));
    }
    // The post-round `Brawl:` grammars are card abilities only and unconditional; the
    // magnitude is the printed amount per anti-support count, which must be positive.
    if let CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerAntiSupport { per_count, .. }
    | CombatStatEffectV1::ReduceOpponentPillzOnVictoryPerAntiSupport { per_count, .. }
    | CombatStatEffectV1::GainPillzOnVictoryPerAntiSupport { per_count, .. } = effect
    {
        if source != CombatStatEffectSourceV1::Ability {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::BrawlPostRoundSource,
            ));
        }
        if per_count == 0 {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::BrawlPostRoundMagnitude,
            ));
        }
        if predicate != CombatStatPredicateV1::Always {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::BrawlPostRoundPredicate,
            ));
        }
        return Ok(());
    }
    // The post-round `Support:` grammars follow the same rule. Their count is the source's
    // own Support context, which `validate_ability_support_context` checks beside the
    // combat-stat Support abilities, so a plan cannot carry a count its hand does not have.
    if let CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerSupport { per_count, .. }
    | CombatStatEffectV1::GainLifeOnVictoryPerSupport { per_count }
    | CombatStatEffectV1::GainPillzOnVictoryPerSupport { per_count } = effect
    {
        let reason = if source != CombatStatEffectSourceV1::Ability {
            Some(InvalidCombatStatPlanReasonV1::SupportPostRoundSource)
        } else if per_count == 0 {
            Some(InvalidCombatStatPlanReasonV1::SupportPostRoundMagnitude)
        } else if predicate != CombatStatPredicateV1::Always {
            Some(InvalidCombatStatPlanReasonV1::SupportPostRoundPredicate)
        } else {
            None
        };
        return match reason {
            Some(reason) => Err(invalid_combat_stat_execute(
                player, hand_slot, source, source_id, reason,
            )),
            None => Ok(()),
        };
    }
    // Backlash is a card ability with a positive magnitude, a Min of at least 1 and no
    // predicate beyond the outcome the engine resolves; the classifier refuses `Min 0`, and
    // the validator keeps a hand-built plan from reaching the self-knockout corner either.
    if let CombatStatEffectV1::ReduceOwnLifeOnVictory { life, minimum } = effect {
        let reason = if source != CombatStatEffectSourceV1::Ability {
            Some(InvalidCombatStatPlanReasonV1::BacklashLifeSource)
        } else if life == 0 || minimum == 0 {
            Some(InvalidCombatStatPlanReasonV1::BacklashLifeMagnitude)
        } else if predicate != CombatStatPredicateV1::Always {
            Some(InvalidCombatStatPlanReasonV1::BacklashLifePredicate)
        } else {
            None
        };
        return match reason {
            Some(reason) => Err(invalid_combat_stat_execute(
                player, hand_slot, source, source_id, reason,
            )),
            None => Ok(()),
        };
    }
    // Corrupt keeps Backlash's guards: a card ability, a positive magnitude, a Min of at least
    // 1 and no predicate; the classifier refuses `Min 0` and the validator keeps a hand-built
    // plan from reaching the self-knockout corner either.
    if let CombatStatEffectV1::ReduceOwnLife { life, minimum } = effect {
        let reason = if source != CombatStatEffectSourceV1::Ability {
            Some(InvalidCombatStatPlanReasonV1::CorruptLifeSource)
        } else if life == 0 || minimum == 0 {
            Some(InvalidCombatStatPlanReasonV1::CorruptLifeMagnitude)
        } else if predicate != CombatStatPredicateV1::Always {
            Some(InvalidCombatStatPlanReasonV1::CorruptLifePredicate)
        } else {
            None
        };
        return match reason {
            Some(reason) => Err(invalid_combat_stat_execute(
                player, hand_slot, source, source_id, reason,
            )),
            None => Ok(()),
        };
    }
    // The capped Defeat Life keeps Defeat Life's guards and adds Heal's: a positive magnitude
    // strictly below a cap, which every printed record satisfies.
    if let CombatStatEffectV1::GainLifeOnDefeatMax { life, maximum } = effect {
        let reason = if source != CombatStatEffectSourceV1::Ability {
            Some(InvalidCombatStatPlanReasonV1::DefeatLifeSource)
        } else if life == 0 || maximum <= life {
            Some(InvalidCombatStatPlanReasonV1::DefeatLifeMagnitude)
        } else if predicate != CombatStatPredicateV1::Always {
            Some(InvalidCombatStatPlanReasonV1::DefeatLifePredicate)
        } else {
            None
        };
        return match reason {
            Some(reason) => Err(invalid_combat_stat_execute(
                player, hand_slot, source, source_id, reason,
            )),
            None => Ok(()),
        };
    }
    // The Defeat opposing Pillz gift shares the Defeat opposing Pillz reduction's guards.
    if let CombatStatEffectV1::GainOpponentPillzOnDefeat { pillz } = effect {
        let reason = if source != CombatStatEffectSourceV1::Ability {
            Some(InvalidCombatStatPlanReasonV1::DefeatOpponentPillzSource)
        } else if pillz == 0 {
            Some(InvalidCombatStatPlanReasonV1::DefeatOpponentPillzMagnitude)
        } else if predicate != CombatStatPredicateV1::Always {
            Some(InvalidCombatStatPlanReasonV1::DefeatOpponentPillzPredicate)
        } else {
            None
        };
        return match reason {
            Some(reason) => Err(invalid_combat_stat_execute(
                player, hand_slot, source, source_id, reason,
            )),
            None => Ok(()),
        };
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
        // must never arrive together on one plan. Revision 73's `Versus` gate (`4887`) is
        // uncapped too.
        if !matches!(
            predicate,
            CombatStatPredicateV1::Always
                | CombatStatPredicateV1::OwnerLostPreviousRound
                | CombatStatPredicateV1::OwnerWonPreviousRound
                | CombatStatPredicateV1::OpponentHandHasClan(_)
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
    if let CombatStatEffectV1::GainLifePerOpponentFinalDamageOnVictory { life_per_damage } = effect
    {
        if source != CombatStatEffectSourceV1::Ability
            || life_per_damage == 0
            || predicate != CombatStatPredicateV1::Always
        {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryLifePerDamageSource,
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
        // Plain, or under the `Unison:` gate.
        if !matches!(
            predicate,
            CombatStatPredicateV1::Always | CombatStatPredicateV1::OwnerHandUnison
        ) {
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
    // `Tune Out` has only been observed as the Cosmohnuts clan bonus; like every other
    // control it carries no predicate, which the rule below enforces.
    if effect == CombatStatEffectV1::SimplifyAttackToPillz
        && source != CombatStatEffectSourceV1::Bonus
    {
        return Err(invalid_combat_stat_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidCombatStatPlanReasonV1::AttackSimplificationSource,
        ));
    }
    // `Damage Impose` is a card ability, unconditional, and on Damage only: `Power Impose`
    // and the prefixed forms have no observed round.
    if let CombatStatEffectV1::ImposePrintedCombatStat { stat } = effect {
        if source != CombatStatEffectSourceV1::Ability
            || stat != CombatStatAttributeV1::Damage
            || predicate != CombatStatPredicateV1::Always
        {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::ConditionalControl,
            ));
        }
        return Ok(());
    }
    // A Stop under one of the conditions the compiler admits by grammar is the one control
    // effect that may carry a predicate; the Reprisal Stop is checked by identity above.
    let conditional_stop = matches!(
        effect,
        CombatStatEffectV1::StopOpponentAbility | CombatStatEffectV1::StopOpponentBonus
    ) && source == CombatStatEffectSourceV1::Ability
        && conditional_stop_predicate_admitted(predicate);
    let conditional_stat_copy = matches!(
        effect,
        CombatStatEffectV1::CopyOpponentPrintedCombatStat { .. }
            | CombatStatEffectV1::ExchangePrintedCombatStat { .. }
            | CombatStatEffectV1::ImposePrintedCombatStat { .. }
    ) && source == CombatStatEffectSourceV1::Ability
        && matches!(
            predicate,
            CombatStatPredicateV1::OwnerMovesFirst
                | CombatStatPredicateV1::OwnerMovesSecond
                | CombatStatPredicateV1::OwnerWonPreviousRound
                | CombatStatPredicateV1::OwnerLostPreviousRound
                | CombatStatPredicateV1::SelectedHandSlotsMatch
                | CombatStatPredicateV1::SelectedHandSlotsDiffer
                | CombatStatPredicateV1::OwnerHandUnison
        )
        // Revision 73: `Versus [clan:..] : Copy: Opp. Damage` (`4956`), a Copy only.
        || (matches!(
            effect,
            CombatStatEffectV1::CopyOpponentPrintedCombatStat { .. }
        ) && source == CombatStatEffectSourceV1::Ability
            && matches!(predicate, CombatStatPredicateV1::OpponentHandHasClan(_)));
    if !matches!(effect, CombatStatEffectV1::ModifyCombatStat { .. })
        && predicate != CombatStatPredicateV1::Always
        && !conditional_stop
        && !conditional_stat_copy
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
    // `Cards` is the one both-sides grammar: a fixed change to Damage or Attack from a card
    // ability, unconditional, never capped, and bounded below exactly when it is a decrease.
    if side == CombatStatAffectedSideV1::Both {
        let admitted = source == CombatStatEffectSourceV1::Ability
            && predicate == CombatStatPredicateV1::Always
            && multiplier == CombatStatMagnitudeV1::Fixed
            && matches!(
                stat,
                CombatStatAttributeV1::Damage | CombatStatAttributeV1::Attack
            )
            && value > 0
            && maximum.is_none()
            && minimum.is_some() == (operation == CombatStatOperationV1::Decrease);
        return if admitted {
            Ok(())
        } else {
            Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::BothCardsModifierShape,
            ))
        };
    }
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
            | CombatStatMagnitudeV1::AntiSupport
            | CombatStatMagnitudeV1::OwnerLife
            | CombatStatMagnitudeV1::OwnerPillz
            | CombatStatMagnitudeV1::OwnerPillzLost
            | CombatStatMagnitudeV1::OwnerLifeLost
    ) && predicate != CombatStatPredicateV1::Always
        && !(multiplier == CombatStatMagnitudeV1::OwnerPillz
            && predicate == CombatStatPredicateV1::OwnerHandUnison
            && source == CombatStatEffectSourceV1::Ability)
        // The owner-clan gate is decided from the owner's own effective clan, and every one
        // of these magnitudes is read independently of it, so the two compose. Ability slot
        // only, and only the gate and magnitudes the server has shown together. Revision 70's
        // `OwnerClanInAnd` is already two conditions and is exempt from nothing here: it
        // carries a fixed magnitude only.
        && !(matches!(predicate, CombatStatPredicateV1::OwnerClanIn(_))
            && source == CombatStatEffectSourceV1::Ability
            && matches!(
                multiplier,
                CombatStatMagnitudeV1::Growth
                    | CombatStatMagnitudeV1::Degrowth
                    | CombatStatMagnitudeV1::OpponentStars
                    | CombatStatMagnitudeV1::AntiSupport
                    | CombatStatMagnitudeV1::OwnerLifeLost
            ))
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
            CombatStatPredicateV1::OwnerMovesFirst
                | CombatStatPredicateV1::OwnerMovesSecond
                | CombatStatPredicateV1::OwnerHandUnison
                | CombatStatPredicateV1::OwnerAbilityStopped
                | CombatStatPredicateV1::OwnerClanIn(_)
                | CombatStatPredicateV1::OpponentHandHasClan(_)
                | CombatStatPredicateV1::OwnerPillzUsedAbove(_)
                | CombatStatPredicateV1::OwnerPillzUsedBelow(_)
                | CombatStatPredicateV1::OwnerWonPreviousRoundAtNight
                | CombatStatPredicateV1::OwnerClanInAnd(..)
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
    if operation == CombatStatOperationV1::Increase
        && maximum.is_some()
        && !(matches!(
            multiplier,
            CombatStatMagnitudeV1::OwnerLife | CombatStatMagnitudeV1::OwnerLifeLost
        ) && matches!(
            stat,
            CombatStatAttributeV1::Power | CombatStatAttributeV1::Damage
        ))
    {
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
    night: bool,
    owner_unison: bool,
    clan: ClanContext,
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
            night,
            owner_unison,
            clan,
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
    night: bool,
    owner_unison: bool,
    clan: ClanContext,
) -> bool {
    match predicate {
        CombatStatPredicateV1::OwnerClanIn(set) => set.contains(clan.owner_effective_clan),
        CombatStatPredicateV1::OwnerPreviousCardClanIn(set) => {
            clan.owner_previous_clan.is_some_and(|id| set.contains(id))
        }
        CombatStatPredicateV1::OpponentHandHasClan(set) => set.0 & clan.opponent_hand_clans != 0,
        CombatStatPredicateV1::OwnerPillzUsedAbove(n) => clan.owner_pillz_used > u16::from(n),
        CombatStatPredicateV1::OwnerPillzUsedBelow(n) => clan.owner_pillz_used < u16::from(n),
        CombatStatPredicateV1::OwnerHandUnison => owner_unison,
        CombatStatPredicateV1::Always => true,
        CombatStatPredicateV1::OwnerMovesFirst => owner == first_mover,
        CombatStatPredicateV1::OwnerMovesSecond => owner != first_mover,
        CombatStatPredicateV1::OwnerWonPreviousRound => previous_round_winner == Some(owner),
        CombatStatPredicateV1::OwnerLostPreviousRound => {
            previous_round_winner == Some(owner.other())
        }
        CombatStatPredicateV1::SelectedHandSlotsMatch => owner_slot == opponent_slot,
        CombatStatPredicateV1::SelectedHandSlotsDiffer => owner_slot != opponent_slot,
        CombatStatPredicateV1::MatchIsNight => night,
        CombatStatPredicateV1::MatchIsDay => !night,
        CombatStatPredicateV1::OwnerWonPreviousRoundAtNight => {
            night && previous_round_winner == Some(owner)
        }
        CombatStatPredicateV1::OwnerClanInAnd(set, conjunct) => {
            set.contains(clan.owner_effective_clan)
                && predicate_matches(
                    conjunct.predicate(),
                    owner,
                    first_mover,
                    owner_slot,
                    opponent_slot,
                    previous_round_winner,
                    night,
                    owner_unison,
                    clan,
                )
        }
        // Construction guarantees no opposing source can stop the owner's ability.
        CombatStatPredicateV1::OwnerAbilityStopped => false,
    }
}

fn prepare_combat_stat_diagnostic(
    validated: ByPlayer<ValidatedSelection>,
    cards: &ByPlayer<[CombatStatCardPlanV1; HAND_SIZE]>,
    first_mover: PlayerId,
    rounds_played: u8,
    previous_round_winner: Option<PlayerId>,
    night: bool,
    life: ByPlayer<u16>,
    pillz: ByPlayer<u16>,
    pillz_lost: ByPlayer<u16>,
    life_lost: ByPlayer<u16>,
    canonical_clans: ByPlayer<[u32; HAND_SIZE]>,
    previous_round_slots: ByPlayer<Option<HandSlot>>,
) -> Result<PreparedCombatResolution, CombatStatDiagnosticErrorV1> {
    let selected = ByPlayer::new(
        cards[PlayerId::P1][validated[PlayerId::P1].slot.index()],
        cards[PlayerId::P2][validated[PlayerId::P2].slot.index()],
    );
    // Brawl reads the *opposing* hand at the *opposing* selected slot, so each player's
    // count is taken from the other player's cards. This is the lowest layer that holds
    // both full hands: `prepare_combat_resolution_with_post_round` below receives only the
    // two selected cards, which is why the number is derived here and carried down rather
    // than recomputed there. When a Copy adopts the opposing Brawl the copier becomes the
    // effect's owner, so the hand it should read is the one its own plan already carries.
    let anti_support = ByPlayer::new(
        effective_clan_character_count(validated[PlayerId::P2].slot, &cards[PlayerId::P2]),
        effective_clan_character_count(validated[PlayerId::P1].slot, &cards[PlayerId::P1]),
    );
    let unison = |player: PlayerId| {
        let clan = cards[player][validated[player].slot.index()].effective_clan_id;
        cards[player]
            .iter()
            .all(|card| card.effective_clan_id == clan)
    };
    let clan = |player: PlayerId| ClanContext {
        owner_effective_clan: cards[player][validated[player].slot.index()].effective_clan_id,
        owner_previous_clan: previous_round_slots[player]
            .map(|slot| canonical_clans[player][slot.index()]),
        opponent_hand_clans: canonical_clans[player.other()]
            .iter()
            .filter(|id| **id < 64)
            .fold(0_u64, |mask, id| mask | (1 << id)),
        owner_pillz_used: validated[player].selection.pillz.saturating_add(1),
    };
    let plans = ByPlayer::new(
        resolution_card_plan(
            selected[PlayerId::P1],
            selected[PlayerId::P2],
            PlayerId::P1,
            first_mover,
            validated[PlayerId::P1].slot,
            validated[PlayerId::P2].slot,
            previous_round_winner,
            anti_support[PlayerId::P1],
            night,
            life[PlayerId::P1],
            pillz[PlayerId::P1],
            pillz_lost[PlayerId::P1],
            life_lost[PlayerId::P1],
            unison(PlayerId::P1),
            clan(PlayerId::P1),
        ),
        resolution_card_plan(
            selected[PlayerId::P2],
            selected[PlayerId::P1],
            PlayerId::P2,
            first_mover,
            validated[PlayerId::P2].slot,
            validated[PlayerId::P1].slot,
            previous_round_winner,
            anti_support[PlayerId::P2],
            night,
            life[PlayerId::P2],
            pillz[PlayerId::P2],
            pillz_lost[PlayerId::P2],
            life_lost[PlayerId::P2],
            unison(PlayerId::P2),
            clan(PlayerId::P2),
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
    night: bool,
    owner_unison: bool,
    clan: ClanContext,
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
                night,
                owner_unison,
                clan,
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
    // Brawl's multiplier, counted over the opposing hand at the opposing selected slot.
    // It is the same number for both of this card's sources and for every magnitude that
    // is not `AntiSupport`, which simply ignores it.
    anti_support_count: u16,
    night: bool,
    owner_life: u16,
    owner_pillz: u16,
    owner_pillz_lost: u16,
    owner_life_lost: u16,
    owner_unison: bool,
    clan: ClanContext,
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
                night,
                owner_unison,
                clan,
            ),
            owner,
            first_mover,
            owner_slot,
            opponent_slot,
            previous_round_winner,
            night,
            owner_unison,
            clan,
        );
        ResolutionSourcePlan {
            effect: effect.and_then(shared_effect),
            post_round: effect.and_then(shared_post_round_effect),
            support_count,
            anti_support_count,
            owner_life,
            owner_pillz,
            owner_pillz_lost,
            owner_life_lost,
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
                CombatStatAffectedSideV1::Both => DiagnosticAffectedSideV1::Both,
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
                CombatStatMagnitudeV1::AntiSupport => DiagnosticMagnitudeV1::AntiSupport,
                CombatStatMagnitudeV1::OpponentDamage => DiagnosticMagnitudeV1::OpponentDamage,
                CombatStatMagnitudeV1::OpponentPower => DiagnosticMagnitudeV1::OpponentPower,
                CombatStatMagnitudeV1::OwnerLife => DiagnosticMagnitudeV1::OwnerLife,
                CombatStatMagnitudeV1::OwnerPillz => DiagnosticMagnitudeV1::OwnerPillz,
                CombatStatMagnitudeV1::OwnerPillzLost => DiagnosticMagnitudeV1::OwnerPillzLost,
                CombatStatMagnitudeV1::OwnerLifeLost => DiagnosticMagnitudeV1::OwnerLifeLost,
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
        CombatStatEffectV1::CancelOpponentResourceModifiers { resources } => {
            DiagnosticCombatEffectV1::CancelOpponentResourceModifiers { resources }
        }
        CombatStatEffectV1::SimplifyAttackToPillz => {
            DiagnosticCombatEffectV1::SimplifyAttackToPillz
        }
        CombatStatEffectV1::ExchangePrintedCombatStat { stat } => {
            DiagnosticCombatEffectV1::ExchangePrintedCombatStat {
                stat: match stat {
                    CombatStatAttributeV1::Attack => DiagnosticCombatStatV1::Attack,
                    CombatStatAttributeV1::Damage => DiagnosticCombatStatV1::Damage,
                    CombatStatAttributeV1::Power => DiagnosticCombatStatV1::Power,
                    CombatStatAttributeV1::PowerAndDamage => DiagnosticCombatStatV1::PowerAndDamage,
                },
            }
        }
        CombatStatEffectV1::ImposePrintedCombatStat { stat } => {
            DiagnosticCombatEffectV1::ImposePrintedCombatStat {
                stat: match stat {
                    CombatStatAttributeV1::Attack => DiagnosticCombatStatV1::Attack,
                    CombatStatAttributeV1::Damage => DiagnosticCombatStatV1::Damage,
                    CombatStatAttributeV1::Power => DiagnosticCombatStatV1::Power,
                    CombatStatAttributeV1::PowerAndDamage => DiagnosticCombatStatV1::PowerAndDamage,
                },
            }
        }
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
        CombatStatEffectV1::RecoverPaidPillzOnDefeat { .. }
        | CombatStatEffectV1::RecoverPaidPillzOnVictory { .. }
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
        | CombatStatEffectV1::GainLifePerOpponentFinalDamageOnVictory { .. }
        | CombatStatEffectV1::GainLifeOnDefeat { .. }
        | CombatStatEffectV1::ReanimateLife { .. }
        | CombatStatEffectV1::GainLifeOnVictoryOrDefeat { .. }
        | CombatStatEffectV1::ReduceOpponentLifeOnVictoryOrDefeat { .. }
        | CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars { .. }
        | CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerAntiSupport { .. }
        | CombatStatEffectV1::ReduceOpponentPillzOnVictoryPerAntiSupport { .. }
        | CombatStatEffectV1::GainPillzOnVictoryPerAntiSupport { .. }
        | CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerSupport { .. }
        | CombatStatEffectV1::GainLifeOnVictoryPerSupport { .. }
        | CombatStatEffectV1::GainPillzOnVictoryPerSupport { .. }
        | CombatStatEffectV1::GainLifeOnVictoryPerOpponentStars { .. }
        | CombatStatEffectV1::GainPillzOnVictoryPerOpponentStars { .. }
        | CombatStatEffectV1::GainPillzAndLifeOnKillshot { .. }
        | CombatStatEffectV1::GainPillzOnKillshot { .. }
        | CombatStatEffectV1::GainLifeOnKillshot { .. }
        | CombatStatEffectV1::ToxinOpponentLifeOnKillshot { .. }
        | CombatStatEffectV1::GainPillzOnDefeat { .. }
        | CombatStatEffectV1::GainPillzAndLifeOnDefeat { .. }
        | CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerRound { .. }
        | CombatStatEffectV1::ReduceOpponentPillzOnVictoryPerRound { .. }
        | CombatStatEffectV1::GainLifeOnVictoryPerRound { .. }
        | CombatStatEffectV1::GainPillzOnVictoryPerRound { .. }
        | CombatStatEffectV1::ReduceOpponentLifeOnDefeat { .. }
        | CombatStatEffectV1::ReduceOpponentLifeOnKillshot { .. }
        | CombatStatEffectV1::ReduceBothPlayersLife { .. }
        | CombatStatEffectV1::GainBothPlayersLifeOnVictoryOrDefeat { .. }
        | CombatStatEffectV1::GainBothPlayersPillzOnVictoryOrDefeat { .. }
        | CombatStatEffectV1::HealLifeOnVictory { .. }
        | CombatStatEffectV1::RegenLifeOnVictory { .. }
        | CombatStatEffectV1::PoisonOpponentLifeOnVictory { .. }
        | CombatStatEffectV1::PoisonOpponentLifeOnDefeat { .. }
        | CombatStatEffectV1::ToxinOpponentLifeOnVictory { .. }
        | CombatStatEffectV1::ConsumeOpponentPillzOnVictory { .. }
        | CombatStatEffectV1::CombustOpponentLifeAndPillzOnVictory { .. }
        | CombatStatEffectV1::DopePillzOnVictory { .. }
        | CombatStatEffectV1::DopePillzOnDefeat { .. }
        | CombatStatEffectV1::GainPillzAndLifeOnVictory { .. }
        | CombatStatEffectV1::GainPillzOnVictoryOrDefeat { .. }
        | CombatStatEffectV1::GainLifePerFinalDamageOnVictoryOrDefeat { .. }
        | CombatStatEffectV1::ReduceOpponentPillzAndLifeOnVictory { .. }
        | CombatStatEffectV1::GainPillzOnVictoryMax { .. }
        | CombatStatEffectV1::ReduceOwnLifeOnVictory { .. }
        | CombatStatEffectV1::ReduceOwnLife { .. }
        | CombatStatEffectV1::GainLifeOnDefeatMax { .. }
        | CombatStatEffectV1::GainOpponentPillzOnDefeat { .. } => return None,
    })
}

pub(crate) fn shared_post_round_effect(
    effect: CombatStatEffectV1,
) -> Option<PostRoundSourceEffect> {
    match effect {
        CombatStatEffectV1::RecoverPaidPillzOnDefeat {
            numerator,
            denominator,
        } => Some(PostRoundSourceEffect::Fixed(
            PostRoundEffect::RecoverPaidPillzOnDefeat {
                numerator,
                denominator,
            },
        )),
        CombatStatEffectV1::RecoverPaidPillzOnVictory {
            numerator,
            denominator,
        } => Some(PostRoundSourceEffect::Fixed(
            PostRoundEffect::RecoverPaidPillzOnVictory {
                numerator,
                denominator,
            },
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
        CombatStatEffectV1::GainLifePerOpponentFinalDamageOnVictory { life_per_damage } => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::GainLifePerOpponentFinalDamageOnVictory { life_per_damage },
            ))
        }
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
        CombatStatEffectV1::GainBothPlayersLifeOnVictoryOrDefeat { life } => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::GainBothPlayersLifeOnVictoryOrDefeat(life),
            ))
        }
        CombatStatEffectV1::GainBothPlayersPillzOnVictoryOrDefeat { pillz } => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::GainBothPlayersPillzOnVictoryOrDefeat(pillz),
            ))
        }
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
        CombatStatEffectV1::PoisonOpponentLifeOnDefeat { life, minimum } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::LatchOnDefeat(
                LatchedEffectV1::PoisonOpponentLife { life, minimum },
            )),
        ),
        CombatStatEffectV1::ToxinOpponentLifeOnVictory { life, minimum } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::LatchOnVictory(
                LatchedEffectV1::ToxinOpponentLife { life, minimum },
            )),
        ),
        CombatStatEffectV1::ConsumeOpponentPillzOnVictory { pillz, minimum } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::LatchOnVictory(
                LatchedEffectV1::ConsumeOpponentPillz { pillz, minimum },
            )),
        ),
        CombatStatEffectV1::CombustOpponentLifeAndPillzOnVictory { amount, minimum } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::LatchOnVictory(
                LatchedEffectV1::CombustOpponentLifeAndPillz { amount, minimum },
            )),
        ),
        CombatStatEffectV1::GainPillzAndLifeOnVictory { amount } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::GainPillzAndLifeOnVictory(amount)),
        ),
        CombatStatEffectV1::GainPillzOnVictoryOrDefeat { pillz } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::GainPillzOnVictoryOrDefeat(pillz)),
        ),
        CombatStatEffectV1::GainPillzOnVictoryMax { pillz, maximum } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::GainPillzOnVictoryMax { pillz, maximum }),
        ),
        CombatStatEffectV1::GainLifePerFinalDamageOnVictoryOrDefeat { life_per_damage } => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::GainLifePerFinalDamageOnVictoryOrDefeat { life_per_damage },
            ))
        }
        CombatStatEffectV1::ReduceOpponentPillzAndLifeOnVictory { amount, minimum } => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::ReduceOpponentPillzAndLifeOnVictory { amount, minimum },
            ))
        }
        CombatStatEffectV1::DopePillzOnVictory { pillz, maximum } => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::LatchOnVictory(LatchedEffectV1::DopePillz { pillz, maximum }),
            ))
        }
        CombatStatEffectV1::DopePillzOnDefeat { pillz, maximum } => {
            Some(PostRoundSourceEffect::Fixed(
                PostRoundEffect::LatchOnDefeat(LatchedEffectV1::DopePillz { pillz, maximum }),
            ))
        }
        CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars { per_star, minimum } => {
            Some(
                PostRoundSourceEffect::ReduceOpponentLifeOnVictoryPerOpponentStars {
                    per_star,
                    minimum,
                },
            )
        }
        CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerAntiSupport { per_count, minimum } => {
            Some(
                PostRoundSourceEffect::ReduceOpponentLifeOnVictoryPerAntiSupport {
                    per_count,
                    minimum,
                },
            )
        }
        CombatStatEffectV1::ReduceOpponentPillzOnVictoryPerAntiSupport { per_count, minimum } => {
            Some(
                PostRoundSourceEffect::ReduceOpponentPillzOnVictoryPerAntiSupport {
                    per_count,
                    minimum,
                },
            )
        }
        CombatStatEffectV1::GainPillzOnVictoryPerAntiSupport { per_count, maximum } => {
            Some(PostRoundSourceEffect::GainPillzOnVictoryPerAntiSupport { per_count, maximum })
        }
        CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerSupport { per_count, minimum } => Some(
            PostRoundSourceEffect::ReduceOpponentLifeOnVictoryPerSupport { per_count, minimum },
        ),
        CombatStatEffectV1::GainLifeOnVictoryPerSupport { per_count } => {
            Some(PostRoundSourceEffect::GainLifeOnVictoryPerSupport { per_count })
        }
        CombatStatEffectV1::GainPillzOnVictoryPerSupport { per_count } => {
            Some(PostRoundSourceEffect::GainPillzOnVictoryPerSupport { per_count })
        }
        CombatStatEffectV1::GainLifeOnVictoryPerOpponentStars { per_star } => {
            Some(PostRoundSourceEffect::GainLifeOnVictoryPerOpponentStars { per_star })
        }
        CombatStatEffectV1::GainPillzOnVictoryPerOpponentStars { per_star } => {
            Some(PostRoundSourceEffect::GainPillzOnVictoryPerOpponentStars { per_star })
        }
        CombatStatEffectV1::GainPillzAndLifeOnKillshot { amount } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::GainPillzAndLifeOnKillshot { amount }),
        ),
        CombatStatEffectV1::GainPillzOnKillshot { pillz } => Some(PostRoundSourceEffect::Fixed(
            PostRoundEffect::GainPillzOnKillshot(pillz),
        )),
        CombatStatEffectV1::GainLifeOnKillshot { life, maximum } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::GainLifeOnKillshot { life, maximum }),
        ),
        CombatStatEffectV1::ToxinOpponentLifeOnKillshot { life, minimum } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::LatchOnKillshot(
                LatchedEffectV1::ToxinOpponentLife { life, minimum },
            )),
        ),
        CombatStatEffectV1::GainPillzOnDefeat { pillz } => Some(PostRoundSourceEffect::Fixed(
            PostRoundEffect::GainPillzOnDefeat(pillz),
        )),
        CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerRound {
            per_round,
            minimum,
            scale,
        } => Some(PostRoundSourceEffect::ReduceOpponentLifeOnVictoryPerRound {
            per_round,
            minimum,
            scale,
        }),
        CombatStatEffectV1::ReduceOpponentPillzOnVictoryPerRound {
            per_round,
            minimum,
            scale,
        } => Some(
            PostRoundSourceEffect::ReduceOpponentPillzOnVictoryPerRound {
                per_round,
                minimum,
                scale,
            },
        ),
        CombatStatEffectV1::GainLifeOnVictoryPerRound { per_round, scale } => {
            Some(PostRoundSourceEffect::GainLifeOnVictoryPerRound { per_round, scale })
        }
        CombatStatEffectV1::GainPillzOnVictoryPerRound { per_round, scale } => {
            Some(PostRoundSourceEffect::GainPillzOnVictoryPerRound { per_round, scale })
        }
        CombatStatEffectV1::GainPillzAndLifeOnDefeat { amount } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::GainPillzAndLifeOnDefeat(amount)),
        ),
        CombatStatEffectV1::ReduceOwnLifeOnVictory { life, minimum } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::ReduceOwnLifeOnVictory { life, minimum }),
        ),
        CombatStatEffectV1::ReduceOwnLife { life, minimum } => Some(PostRoundSourceEffect::Fixed(
            PostRoundEffect::ReduceOwnLife { life, minimum },
        )),
        CombatStatEffectV1::GainLifeOnDefeatMax { life, maximum } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::GainLifeOnDefeatMax { life, maximum }),
        ),
        CombatStatEffectV1::GainOpponentPillzOnDefeat { pillz } => Some(
            PostRoundSourceEffect::Fixed(PostRoundEffect::GainOpponentPillzOnDefeat(pillz)),
        ),
        CombatStatEffectV1::ModifyCombatStat { .. }
        | CombatStatEffectV1::StopOpponentAbility
        | CombatStatEffectV1::StopOpponentBonus
        | CombatStatEffectV1::CancelOpponentCombatStatModifiers { .. }
        | CombatStatEffectV1::ProtectOwnCombatStat { .. }
        | CombatStatEffectV1::ProtectOwnAbility
        | CombatStatEffectV1::ProtectOwnBonus
        | CombatStatEffectV1::CopyOpponentPrintedCombatStat { .. }
        | CombatStatEffectV1::ExchangePrintedCombatStat { .. }
        | CombatStatEffectV1::ImposePrintedCombatStat { .. }
        | CombatStatEffectV1::CancelOpponentResourceModifiers { .. }
        | CombatStatEffectV1::SimplifyAttackToPillz => None,
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

    const TWO_OF_THREE: CombatStatEffectV1 = CombatStatEffectV1::RecoverPaidPillzOnDefeat {
        numerator: 2,
        denominator: 3,
    };

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
        let spec = spec_with_p1(CombatStatEffectSourceV1::Ability, 729, TWO_OF_THREE, 7);
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

    /// The recovered amount is `max(1, floor((paid + 1) * N / M))`: the Pillz placed on the
    /// card, free pill included, rounded down. For 2/3 and 1/2 that equals rounding the paid
    /// cost up, which is what the Defeat captures show; 1/3 is where the two differ.
    #[test]
    fn recovery_ratio_rounds_down_over_the_pillz_placed_including_the_free_pill() {
        let defeat = |numerator, denominator| CombatStatEffectV1::RecoverPaidPillzOnDefeat {
            numerator,
            denominator,
        };
        let victory = |numerator, denominator| CombatStatEffectV1::RecoverPaidPillzOnVictory {
            numerator,
            denominator,
        };
        let play = |effect, pillz, fury, win: bool| {
            let mut spec = spec_with_p1(CombatStatEffectSourceV1::Ability, 1, effect, 20);
            spec.base_rules.players[PlayerId::P1].hand[0].power = if win { 400 } else { 1 };
            let mut game = CombatStatDiagnosticV1::new(spec).unwrap();
            let (report, _) = game.make(input(pillz, fury)).unwrap();
            assert_eq!(report.cards[PlayerId::P1].won, win);
            report.players[PlayerId::P1].pillz
        };
        let cost = |pillz: u16, fury: bool| pillz + if fury { 3 } else { 0 };
        for (effect, pillz, fury, win, recovered) in [
            // 1207064/0: Bubbles bets 5 and loses; 12 - 5 + 3 = 10.
            (defeat(1, 2), 5, false, false, 3),
            // 1131463/1: Morgane bets 6 and loses; 10 - 6 + 3 = 7.
            (defeat(1, 2), 6, false, false, 3),
            // 877983/1 and 1145886/1: a bet of nothing still recovers one.
            (defeat(1, 2), 0, false, false, 1),
            (defeat(1, 2), 4, true, false, 4),
            // 947010/0: Kyrioz Ld bets 7 and wins; 12 - 7 + 2 = 7, not the 3 rounding up gives.
            (victory(1, 3), 7, false, true, 2),
            // 1093500/3, 946570/0, 1025563/0: 9, 6 and 3 recover 3, 2 and 1.
            (victory(1, 3), 9, false, true, 3),
            (victory(1, 3), 6, false, true, 2),
            (victory(1, 3), 3, false, true, 1),
            (victory(1, 3), 0, false, true, 1),
            (victory(1, 3), 2, true, true, 2),
            // 1025181/0: Porcusite's Unison 1/2 bets 10 and wins; 12 - 10 + 5 = 7.
            (victory(1, 2), 10, false, true, 5),
            // A Recover on the other outcome pays nothing.
            (victory(1, 3), 6, false, false, 0),
            (defeat(1, 2), 6, false, true, 0),
        ] {
            assert_eq!(
                play(effect, pillz, fury, win),
                20 - cost(pillz, fury) + recovered,
                "{effect:?} bet {pillz} fury {fury} win {win}"
            );
        }
    }

    #[test]
    fn unison_recovery_pays_only_a_one_clan_hand() {
        let effect = CombatStatEffectV1::RecoverPaidPillzOnVictory {
            numerator: 1,
            denominator: 3,
        };
        let play = |unison: bool| {
            let mut spec = spec_with_p1(CombatStatEffectSourceV1::Ability, 3752, effect, 20);
            spec.base_rules.players[PlayerId::P1].hand[0].power = 400;
            spec.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
                source_id: 3752,
                predicate: CombatStatPredicateV1::OwnerHandUnison,
                effect,
            };
            if unison {
                for card in &mut spec.cards[PlayerId::P1] {
                    card.effective_clan_id = 31;
                }
            }
            let mut game = CombatStatDiagnosticV1::new(spec).unwrap();
            game.make(input(6, false)).unwrap().0.players[PlayerId::P1].pillz
        };
        // 1093079/0: Cynosine bets 6 in an all-Bangers hand and wins; 12 - 6 + 2 = 8.
        assert_eq!(play(true), 16);
        assert_eq!(play(false), 14);
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
        let mut spec = spec_with_p1(CombatStatEffectSourceV1::Bonus, 577, TWO_OF_THREE, 3);
        spec.cards[PlayerId::P2][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1,
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::StopOpponentBonus,
        };
        let mut stopped = CombatStatDiagnosticV1::new(spec).unwrap();
        let (report, _) = stopped.make(input(3, false)).unwrap();
        assert_eq!(report.players[PlayerId::P1].pillz, 0);

        let mut spec = spec_with_p1(CombatStatEffectSourceV1::Ability, 1418, TWO_OF_THREE, 3);
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
            TWO_OF_THREE,
            0,
        ))
        .unwrap();
        let (report, _) = minimum.make(input(0, false)).unwrap();
        assert_eq!(report.players[PlayerId::P1].pillz, 1);

        // The two independently active sources represent two END events, so each exact
        // recovery applies after the same paid cost.
        let mut double = spec_with_p1(CombatStatEffectSourceV1::Ability, 729, TWO_OF_THREE, 3);
        double.cards[PlayerId::P1][0].bonus = CombatStatSourcePlanV1::Execute {
            source_id: 577,
            predicate: CombatStatPredicateV1::Always,
            effect: TWO_OF_THREE,
        };
        double.cards[PlayerId::P1][0].source_bonus_support_count = 1;
        let mut double = CombatStatDiagnosticV1::new(double).unwrap();
        let (report, _) = double.make(input(3, false)).unwrap();
        assert_eq!(report.players[PlayerId::P1].pillz, 4);
    }

    #[test]
    fn defeat_recovery_requires_a_round_loss_and_still_runs_after_a_tie_break_or_ko() {
        let mut winning_spec =
            spec_with_p1(CombatStatEffectSourceV1::Ability, 1418, TWO_OF_THREE, 3);
        winning_spec.base_rules.players[PlayerId::P1].hand[0].power = 40;
        let mut winner = CombatStatDiagnosticV1::new(winning_spec).unwrap();
        let (report, _) = winner.make(input(3, false)).unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P1].pillz, 0);

        let mut tied_spec = spec_with_p1(CombatStatEffectSourceV1::Ability, 1418, TWO_OF_THREE, 0);
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

        let mut ko_spec = spec_with_p1(CombatStatEffectSourceV1::Ability, 1418, TWO_OF_THREE, 3);
        ko_spec.base_rules.players[PlayerId::P1].initial_life = 2;
        let mut ko = CombatStatDiagnosticV1::new(ko_spec).unwrap();
        let (report, _) = ko.make(input(3, false)).unwrap();
        assert_eq!(report.players[PlayerId::P1].life, 0);
        assert_eq!(report.players[PlayerId::P1].pillz, 2);
        assert_eq!(report.status, MatchStatus::Won(PlayerId::P2));
    }

    #[test]
    fn recovery_public_plans_are_bounded_by_grammar_and_overflow_is_atomic() {
        // Sasl Lovelace's same-text `2475` was locked out until revision 63.
        assert!(CombatStatDiagnosticV1::new(spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            2475,
            TWO_OF_THREE,
            3,
        ))
        .is_ok());
        let defeat = |numerator, denominator| CombatStatEffectV1::RecoverPaidPillzOnDefeat {
            numerator,
            denominator,
        };
        let victory = |numerator, denominator| CombatStatEffectV1::RecoverPaidPillzOnVictory {
            numerator,
            denominator,
        };
        for effect in [
            defeat(0, 3),
            defeat(3, 3),
            defeat(1, 0),
            victory(0, 0),
            victory(4, 3),
        ] {
            assert!(
                matches!(
                    CombatStatDiagnosticV1::new(spec_with_p1(
                        CombatStatEffectSourceV1::Ability,
                        2475,
                        effect,
                        3,
                    )),
                    Err(CombatStatPlanErrorV1::InvalidExecute {
                        reason: InvalidCombatStatPlanReasonV1::RecoveryRatio,
                        ..
                    })
                ),
                "{effect:?}"
            );
        }
        assert!(matches!(
            CombatStatDiagnosticV1::new(spec_with_p1(
                CombatStatEffectSourceV1::Bonus,
                3459,
                victory(1, 3),
                3,
            )),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::RecoverySource,
                ..
            })
        ));
        let mut unison_defeat =
            spec_with_p1(CombatStatEffectSourceV1::Ability, 770, defeat(1, 2), 3);
        unison_defeat.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 770,
            predicate: CombatStatPredicateV1::OwnerHandUnison,
            effect: defeat(1, 2),
        };
        assert!(matches!(
            CombatStatDiagnosticV1::new(unison_defeat),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::RecoveryPredicate,
                ..
            })
        ));

        let mut wrong_predicate =
            spec_with_p1(CombatStatEffectSourceV1::Ability, 1418, TWO_OF_THREE, 3);
        wrong_predicate.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1418,
            predicate: CombatStatPredicateV1::OwnerLostPreviousRound,
            effect: TWO_OF_THREE,
        };
        assert!(matches!(
            CombatStatDiagnosticV1::new(wrong_predicate),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::RecoveryPredicate,
                ..
            })
        ));

        let mut overflow = CombatStatDiagnosticV1::new(spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1418,
            TWO_OF_THREE,
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

    fn execute(effect: CombatStatEffectV1, id: u32) -> CombatStatSourcePlanV1 {
        CombatStatSourcePlanV1::Execute {
            source_id: id,
            predicate: CombatStatPredicateV1::Always,
            effect,
        }
    }

    fn cards_damage(operation: CombatStatOperationV1, minimum: Option<u16>) -> CombatStatEffectV1 {
        CombatStatEffectV1::ModifyCombatStat {
            side: CombatStatAffectedSideV1::Both,
            stat: CombatStatAttributeV1::Damage,
            operation,
            value: 2,
            minimum,
            maximum: None,
            multiplier: CombatStatMagnitudeV1::Fixed,
        }
    }

    #[test]
    fn cards_lands_on_both_cards_with_the_own_half_before_opposing_reductions() {
        // 1079078/3: Rajesh (5/6) prints `-2 Cards Damage, Min 4` against Sue (6/3), whose
        // `-1 Opp Power And Damage, Min 3` takes his Power to 4. His own Damage is at 4 before
        // Sue's reduction takes it to the reported 3 - the other order leaves 4 - and Sue's
        // 3, already under Min 4, is left alone rather than pulled up to it.
        let mut spec = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            3570,
            cards_damage(CombatStatOperationV1::Decrease, Some(4)),
            0,
        );
        spec.base_rules.players[PlayerId::P1].hand[0].power = 5;
        spec.base_rules.players[PlayerId::P1].hand[0].damage = 6;
        spec.base_rules.players[PlayerId::P2].hand[0].power = 6;
        spec.cards[PlayerId::P2][0].ability = execute(
            CombatStatEffectV1::ModifyCombatStat {
                side: CombatStatAffectedSideV1::Opponent,
                stat: CombatStatAttributeV1::PowerAndDamage,
                operation: CombatStatOperationV1::Decrease,
                value: 1,
                minimum: Some(3),
                maximum: None,
                multiplier: CombatStatMagnitudeV1::Fixed,
            },
            916,
        );
        let mut game = CombatStatDiagnosticV1::new(spec).unwrap();
        let before = game.position().clone();
        let (report, undo) = game.make(input(0, false)).unwrap();
        assert_eq!(report.cards[PlayerId::P1].power, 4);
        assert_eq!(report.cards[PlayerId::P1].damage, 3);
        assert_eq!(report.cards[PlayerId::P2].damage, 3);
        assert!(report.cards[PlayerId::P2].won);
        assert_eq!(report.players[PlayerId::P1].life, 17);
        game.unmake(undo);
        assert_eq!(game.position(), &before);
    }

    #[test]
    fn cards_increase_passes_protection_and_an_opposing_cancel_or_copy_is_refused() {
        // 874795/0: `Cards Damage +2` takes El Resbaladizo 6 to 8 and Aurora 5 to 7. The
        // opposing half is an increase, which Protection - a refusal of reductions by the
        // opposing character, in the site's own words - does not touch.
        let mut spec = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            3295,
            cards_damage(CombatStatOperationV1::Increase, None),
            4,
        );
        spec.cards[PlayerId::P2][0].bonus = execute(
            CombatStatEffectV1::ProtectOwnCombatStat {
                stat: CombatStatAttributeV1::PowerAndDamage,
            },
            1355,
        );
        spec.cards[PlayerId::P2][0].source_bonus_support_count = 1;
        let mut game = CombatStatDiagnosticV1::new(spec.clone()).unwrap();
        let (report, _) = game.make(input(0, false)).unwrap();
        assert_eq!(report.cards[PlayerId::P1].damage, 5);
        assert_eq!(report.cards[PlayerId::P2].damage, 5);

        // The decrease's opposing half is a reduction by the opposing character, so the same
        // Protection refuses it while the owner's own half still lands.
        spec.cards[PlayerId::P1][0].ability =
            execute(cards_damage(CombatStatOperationV1::Decrease, Some(1)), 2018);
        let mut game = CombatStatDiagnosticV1::new(spec.clone()).unwrap();
        let (report, _) = game.make(input(0, false)).unwrap();
        assert_eq!(report.cards[PlayerId::P1].damage, 1);
        assert_eq!(report.cards[PlayerId::P2].damage, 3);

        // No round shows a `Cards` modifier meeting an opposing cancel of its stat or an
        // opposing Copy, so either refuses the match; an opposing cancel of another stat does
        // not meet it.
        let refused = |spec: CombatStatDiagnosticMatchSpecV1| {
            matches!(
                CombatStatDiagnosticV1::new(spec),
                Err(CombatStatPlanErrorV1::InvalidExecute {
                    reason: InvalidCombatStatPlanReasonV1::BothCardsModifierAgainstUnpinnedEffect,
                    ..
                })
            )
        };
        spec.cards[PlayerId::P1][0].ability =
            execute(cards_damage(CombatStatOperationV1::Increase, None), 3295);
        for (stat, refuses) in [
            (CombatStatAttributeV1::Damage, true),
            (CombatStatAttributeV1::PowerAndDamage, true),
            (CombatStatAttributeV1::Power, false),
            (CombatStatAttributeV1::Attack, false),
        ] {
            let mut cancelled = spec.clone();
            cancelled.cards[PlayerId::P2][0].bonus = execute(
                CombatStatEffectV1::CancelOpponentCombatStatModifiers { stat },
                4414,
            );
            assert_eq!(refused(cancelled), refuses, "{stat:?}");
        }
        let mut copied = spec;
        copied.cards[PlayerId::P2][2].ability = CombatStatSourcePlanV1::CopyOpponentSource {
            source_id: 2918,
            copied: CopiedSourceKindV1::Ability,
            predicate: CombatStatPredicateV1::Always,
        };
        copied.cards[PlayerId::P2][2].source_ability_support_count = 1;
        assert!(refused(copied));
    }

    #[test]
    fn cards_attack_reduces_both_attacks_each_to_its_own_floor() {
        // 1078555/1: Miss Denna's `-7 Cards Attack, Min 0` takes her own 7 to 0 and Callie's
        // 36 + 12 Support to 41.
        let mut spec = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            4616,
            CombatStatEffectV1::ModifyCombatStat {
                side: CombatStatAffectedSideV1::Both,
                stat: CombatStatAttributeV1::Attack,
                operation: CombatStatOperationV1::Decrease,
                value: 7,
                minimum: Some(0),
                maximum: None,
                multiplier: CombatStatMagnitudeV1::Fixed,
            },
            0,
        );
        spec.base_rules.players[PlayerId::P1].hand[0].power = 7;
        spec.base_rules.players[PlayerId::P2].hand[0].power = 48;
        let mut game = CombatStatDiagnosticV1::new(spec).unwrap();
        let (report, _) = game.make(input(0, false)).unwrap();
        assert_eq!(report.cards[PlayerId::P1].attack, 0);
        assert_eq!(report.cards[PlayerId::P2].attack, 41);
    }

    #[test]
    fn a_cards_plan_outside_its_one_shape_is_refused() {
        let decrease = cards_damage(CombatStatOperationV1::Decrease, Some(1));
        let refused = |spec: CombatStatDiagnosticMatchSpecV1| {
            matches!(
                CombatStatDiagnosticV1::new(spec),
                Err(CombatStatPlanErrorV1::InvalidExecute {
                    reason: InvalidCombatStatPlanReasonV1::BothCardsModifierShape,
                    ..
                })
            )
        };
        assert!(refused(spec_with_p1(
            CombatStatEffectSourceV1::Bonus,
            2018,
            decrease,
            0
        )));
        let mut conditional = spec_with_p1(CombatStatEffectSourceV1::Ability, 2018, decrease, 0);
        conditional.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 2018,
            predicate: CombatStatPredicateV1::OwnerWonPreviousRound,
            effect: decrease,
        };
        assert!(refused(conditional));
        for effect in [
            cards_damage(CombatStatOperationV1::Decrease, None),
            cards_damage(CombatStatOperationV1::Increase, Some(1)),
            CombatStatEffectV1::ModifyCombatStat {
                side: CombatStatAffectedSideV1::Both,
                stat: CombatStatAttributeV1::Power,
                operation: CombatStatOperationV1::Decrease,
                value: 2,
                minimum: Some(2),
                maximum: None,
                multiplier: CombatStatMagnitudeV1::Fixed,
            },
            CombatStatEffectV1::ModifyCombatStat {
                side: CombatStatAffectedSideV1::Both,
                stat: CombatStatAttributeV1::Damage,
                operation: CombatStatOperationV1::Increase,
                value: 2,
                minimum: None,
                maximum: None,
                multiplier: CombatStatMagnitudeV1::Growth,
            },
        ] {
            assert!(
                refused(spec_with_p1(
                    CombatStatEffectSourceV1::Ability,
                    2018,
                    effect,
                    0
                )),
                "{effect:?}"
            );
        }
    }

    #[test]
    fn tune_out_makes_each_attack_its_bet_and_ignores_every_attack_modifier() {
        // P2 prints 31 Power and an own `Attack +30`; under P1's Tune Out both Powers are 1,
        // P1's bet of 4 is an Attack of 5 against P2's 1, and Fury adds Damage only.
        let mut spec = spec_with_p1(
            CombatStatEffectSourceV1::Bonus,
            3496,
            CombatStatEffectV1::SimplifyAttackToPillz,
            7,
        );
        spec.cards[PlayerId::P2][0].ability = execute(
            CombatStatEffectV1::ModifyCombatStat {
                side: CombatStatAffectedSideV1::Player,
                stat: CombatStatAttributeV1::Attack,
                operation: CombatStatOperationV1::Increase,
                value: 30,
                minimum: None,
                maximum: None,
                multiplier: CombatStatMagnitudeV1::Fixed,
            },
            5,
        );
        let mut game = CombatStatDiagnosticV1::new(spec.clone()).unwrap();
        let before = game.position().clone();
        let (report, undo) = game.make(input(4, true)).unwrap();
        assert_eq!(report.cards[PlayerId::P1].power, 1);
        assert_eq!(report.cards[PlayerId::P2].power, 1);
        assert_eq!(report.cards[PlayerId::P1].attack, 5);
        assert_eq!(report.cards[PlayerId::P2].attack, 1);
        assert_eq!(report.cards[PlayerId::P1].damage, 5);
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P2].life, 15);
        game.unmake(undo);
        assert_eq!(game.position(), &before);

        // Equal bets fall to the ordinary tie-break (925781/0).
        let mut game = CombatStatDiagnosticV1::new(spec.clone()).unwrap();
        let (report, _) = game.make(input(0, false)).unwrap();
        assert_eq!(report.cards[PlayerId::P1].attack, 1);
        assert_eq!(report.cards[PlayerId::P2].attack, 1);
        assert!(report.cards[PlayerId::P1].won);

        // A stopped Tune Out leaves an ordinary round (924146/3).
        spec.cards[PlayerId::P2][0].ability = execute(CombatStatEffectV1::StopOpponentBonus, 1359);
        let mut game = CombatStatDiagnosticV1::new(spec).unwrap();
        let (report, _) = game.make(input(4, false)).unwrap();
        assert_eq!(report.cards[PlayerId::P1].attack, 30);
        assert_eq!(report.cards[PlayerId::P2].attack, 31);
    }

    #[test]
    fn tune_out_is_bonus_only_and_refused_beside_a_killshot_or_an_opposing_power_cancel() {
        let refused = |spec: CombatStatDiagnosticMatchSpecV1,
                       expected: InvalidCombatStatPlanReasonV1| {
            matches!(
                CombatStatDiagnosticV1::new(spec),
                Err(CombatStatPlanErrorV1::InvalidExecute { reason, .. }) if reason == expected
            )
        };
        assert!(refused(
            spec_with_p1(
                CombatStatEffectSourceV1::Ability,
                3496,
                CombatStatEffectV1::SimplifyAttackToPillz,
                0
            ),
            InvalidCombatStatPlanReasonV1::AttackSimplificationSource,
        ));
        let tune_out = || {
            spec_with_p1(
                CombatStatEffectSourceV1::Bonus,
                3496,
                CombatStatEffectV1::SimplifyAttackToPillz,
                0,
            )
        };
        let killshot = CombatStatEffectV1::ReduceOpponentLifeOnKillshot {
            life: 3,
            minimum: 0,
        };
        // An opposing Killshot anywhere in the other hand, an opposing Power cancel, and a
        // Killshot printed beside the Tune Out bonus on its own card.
        let mut opposing_killshot = tune_out();
        opposing_killshot.cards[PlayerId::P2][3].ability = execute(killshot, 4459);
        assert!(refused(
            opposing_killshot,
            InvalidCombatStatPlanReasonV1::AttackSimplificationAgainstUnpinnedEffect,
        ));
        let mut opposing_cancel = tune_out();
        opposing_cancel.cards[PlayerId::P2][2].ability = execute(
            CombatStatEffectV1::CancelOpponentCombatStatModifiers {
                stat: CombatStatAttributeV1::Power,
            },
            1164,
        );
        assert!(refused(
            opposing_cancel,
            InvalidCombatStatPlanReasonV1::AttackSimplificationAgainstUnpinnedEffect,
        ));
        let mut own_killshot = tune_out();
        own_killshot.cards[PlayerId::P1][0].ability = execute(killshot, 4459);
        assert!(refused(
            own_killshot,
            InvalidCombatStatPlanReasonV1::AttackSimplificationAgainstUnpinnedEffect,
        ));
        // An opposing Copy, and a Power reduction to Min 0 on either side - no round shows
        // whether it lands before or after Power is set to 1.
        let mut opposing_copy = tune_out();
        opposing_copy.cards[PlayerId::P2][2].ability = CombatStatSourcePlanV1::CopyOpponentSource {
            source_id: 2918,
            copied: CopiedSourceKindV1::Bonus,
            predicate: CombatStatPredicateV1::Always,
        };
        opposing_copy.cards[PlayerId::P2][2].source_ability_support_count = 1;
        assert!(refused(
            opposing_copy,
            InvalidCombatStatPlanReasonV1::AttackSimplificationAgainstUnpinnedEffect,
        ));
        for (owner, minimum, refuses) in [
            (PlayerId::P2, 0, true),
            (PlayerId::P1, 0, true),
            (PlayerId::P2, 1, false),
        ] {
            let mut reduced = tune_out();
            reduced.cards[owner][3].ability = execute(
                CombatStatEffectV1::ModifyCombatStat {
                    side: CombatStatAffectedSideV1::Opponent,
                    stat: CombatStatAttributeV1::Power,
                    operation: CombatStatOperationV1::Decrease,
                    value: 3,
                    minimum: Some(minimum),
                    maximum: None,
                    multiplier: CombatStatMagnitudeV1::Fixed,
                },
                1815,
            );
            assert_eq!(
                refused(
                    reduced,
                    InvalidCombatStatPlanReasonV1::AttackSimplificationAgainstUnpinnedEffect,
                ),
                refuses,
                "{owner:?} Min {minimum}"
            );
        }
        // An opposing Damage cancel does not meet it: Tune Out leaves Damage alone.
        let mut damage_cancel = tune_out();
        damage_cancel.cards[PlayerId::P2][2].ability = execute(
            CombatStatEffectV1::CancelOpponentCombatStatModifiers {
                stat: CombatStatAttributeV1::Damage,
            },
            4414,
        );
        assert!(CombatStatDiagnosticV1::new(damage_cancel).is_ok());
    }

    /// Round two: both players play slot 1, nobody bets, P2 moves first and wins 6 x 1
    /// against 31 x 1 while dealing 3.
    fn second_round() -> BaseRulesRoundInput {
        BaseRulesRoundInput {
            first_mover: PlayerId::P2,
            selections: ByPlayer::new(
                BaseRulesSelection::new(1, 0, false),
                BaseRulesSelection::new(1, 0, false),
            ),
        }
    }

    fn killshot_game(effect: CombatStatEffectV1, p1_pillz: u16) -> CombatStatDiagnosticV1 {
        CombatStatDiagnosticV1::new(spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            2250,
            effect,
            p1_pillz,
        ))
        .unwrap()
    }

    /// The Killshot own gains and the Toxin latch share revision 38's trigger: P1's 6 power
    /// against P2's 31 doubles at ten Pillz (66 against 31) and only wins at five (36).
    #[test]
    fn killshot_own_gains_and_toxin_latch_pay_on_the_attack_ratio_alone() {
        let pillz = CombatStatEffectV1::GainPillzOnKillshot { pillz: 3 };
        let (report, _) = killshot_game(pillz, 10).make(input(10, false)).unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P1].pillz, 3, "10 - 10 + 3");
        let (report, _) = killshot_game(pillz, 10).make(input(5, false)).unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(
            report.players[PlayerId::P1].pillz,
            5,
            "a plain win pays nothing"
        );

        let life = CombatStatEffectV1::GainLifeOnKillshot {
            life: 3,
            maximum: 0,
        };
        let (report, _) = killshot_game(life, 10).make(input(10, false)).unwrap();
        assert_eq!(report.players[PlayerId::P1].life, 23);
        let (report, _) = killshot_game(life, 10).make(input(5, false)).unwrap();
        assert_eq!(report.players[PlayerId::P1].life, 20);

        // Heal's cap: an overshoot stops at Max, and an owner at or past Max gains nothing.
        let capped = CombatStatEffectV1::GainLifeOnKillshot {
            life: 5,
            maximum: 22,
        };
        for (start, expected) in [(20, 22), (18, 22), (16, 21), (22, 22), (24, 24)] {
            let mut spec = spec_with_p1(CombatStatEffectSourceV1::Ability, 5065, capped, 10);
            spec.base_rules.players[PlayerId::P1].initial_life = start;
            let (report, _) = CombatStatDiagnosticV1::new(spec)
                .unwrap()
                .make(input(10, false))
                .unwrap();
            assert_eq!(report.players[PlayerId::P1].life, expected, "from {start}");
        }

        // Toxin latches on the ratio and pays at once: 20 - 3 combat damage - 1, then one
        // more in a round P1 loses. A plain win latches nothing.
        let toxin = CombatStatEffectV1::ToxinOpponentLifeOnKillshot {
            life: 1,
            minimum: 0,
        };
        let mut latched = killshot_game(toxin, 10);
        let (report, _) = latched.make(input(10, false)).unwrap();
        assert_eq!(report.players[PlayerId::P2].life, 16);
        let before = latched.position().clone();
        let (report, undo) = latched.make(second_round()).unwrap();
        assert!(!report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P2].life, 15);
        latched.unmake(undo);
        assert_eq!(latched.position(), &before);
        let mut plain = killshot_game(toxin, 10);
        let (report, _) = plain.make(input(5, false)).unwrap();
        assert_eq!(report.players[PlayerId::P2].life, 17);
        let (report, _) = plain.make(second_round()).unwrap();
        assert_eq!(report.players[PlayerId::P2].life, 17);

        // At zero attack on both sides the ratio holds for the side that loses the tie, as
        // it does for revision 38's reduction: a living loser gains, a knocked-out one does
        // not, and the latch is taken either way.
        let stalled = |effect, p1_life| {
            let mut spec = spec_with_p1(CombatStatEffectSourceV1::Ability, 2250, effect, 0);
            spec.base_rules.players[PlayerId::P1].hand[0].power = 0;
            spec.base_rules.players[PlayerId::P2].hand[0].power = 0;
            spec.base_rules.players[PlayerId::P1].initial_life = p1_life;
            let mut game = CombatStatDiagnosticV1::new(spec).unwrap();
            let (report, _) = game
                .make(BaseRulesRoundInput {
                    first_mover: PlayerId::P2,
                    selections: ByPlayer::new(
                        BaseRulesSelection::new(0, 0, false),
                        BaseRulesSelection::new(0, 0, false),
                    ),
                })
                .unwrap();
            assert!(!report.cards[PlayerId::P1].won);
            report
        };
        assert_eq!(stalled(pillz, 20).players[PlayerId::P1].pillz, 3);
        assert_eq!(stalled(life, 20).players[PlayerId::P1].life, 20);
        assert_eq!(stalled(life, 3).players[PlayerId::P1].life, 0);
        assert_eq!(stalled(pillz, 3).players[PlayerId::P1].pillz, 0);
        assert_eq!(stalled(toxin, 3).players[PlayerId::P2].life, 19);
    }

    #[test]
    fn killshot_post_round_plans_are_ability_only_positive_and_only_unison_gates_life() {
        let life = CombatStatEffectV1::GainLifeOnKillshot {
            life: 4,
            maximum: 0,
        };
        let plan = |effect, predicate| CombatStatSourcePlanV1::Execute {
            source_id: 3894,
            predicate,
            effect,
        };
        let reason = |spec| match CombatStatDiagnosticV1::new(spec) {
            Err(CombatStatPlanErrorV1::InvalidExecute { reason, .. }) => Some(reason),
            _ => None,
        };
        for effect in [
            CombatStatEffectV1::GainPillzOnKillshot { pillz: 3 },
            life,
            CombatStatEffectV1::ToxinOpponentLifeOnKillshot {
                life: 1,
                minimum: 0,
            },
        ] {
            assert!(CombatStatDiagnosticV1::new(spec_with_p1(
                CombatStatEffectSourceV1::Ability,
                3894,
                effect,
                10
            ))
            .is_ok());
            assert_eq!(
                reason(spec_with_p1(
                    CombatStatEffectSourceV1::Bonus,
                    3894,
                    effect,
                    10
                )),
                Some(InvalidCombatStatPlanReasonV1::KillshotPostRoundSource)
            );
        }
        let zero = CombatStatEffectV1::GainPillzOnKillshot { pillz: 0 };
        assert_eq!(
            reason(spec_with_p1(
                CombatStatEffectSourceV1::Ability,
                2250,
                zero,
                10
            )),
            Some(InvalidCombatStatPlanReasonV1::KillshotPostRoundMagnitude)
        );
        // `Unison:` is the one predicate printed, and only on the uncapped Life gain.
        for (effect, predicate, admitted) in [
            (life, CombatStatPredicateV1::OwnerHandUnison, true),
            (life, CombatStatPredicateV1::OwnerWonPreviousRound, false),
            (
                CombatStatEffectV1::GainLifeOnKillshot {
                    life: 5,
                    maximum: 14,
                },
                CombatStatPredicateV1::OwnerHandUnison,
                false,
            ),
            (
                CombatStatEffectV1::GainPillzOnKillshot { pillz: 3 },
                CombatStatPredicateV1::OwnerHandUnison,
                false,
            ),
            (
                CombatStatEffectV1::ToxinOpponentLifeOnKillshot {
                    life: 1,
                    minimum: 0,
                },
                CombatStatPredicateV1::OwnerHandUnison,
                false,
            ),
        ] {
            let mut spec = spec_with_p1(CombatStatEffectSourceV1::Ability, 3894, effect, 10);
            spec.cards[PlayerId::P1][0].ability = plan(effect, predicate);
            let expected =
                (!admitted).then_some(InvalidCombatStatPlanReasonV1::KillshotPostRoundPredicate);
            assert_eq!(reason(spec), expected, "{effect:?} under {predicate:?}");
        }

        // The gate itself: a mixed hand never pays, a mono-clan one pays on the ratio.
        for (mono, expected) in [(false, 20), (true, 24)] {
            let mut spec = spec_with_p1(CombatStatEffectSourceV1::Ability, 3894, life, 10);
            spec.cards[PlayerId::P1][0].ability =
                plan(life, CombatStatPredicateV1::OwnerHandUnison);
            if mono {
                for card in spec.cards[PlayerId::P1].iter_mut() {
                    card.effective_clan_id = 1;
                }
            }
            let (report, _) = CombatStatDiagnosticV1::new(spec)
                .unwrap()
                .make(input(10, false))
                .unwrap();
            assert_eq!(report.players[PlayerId::P1].life, expected, "mono {mono}");
        }
    }
}
