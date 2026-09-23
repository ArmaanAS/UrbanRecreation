//! The current, replay-grounded engine.
//!
//! This module intentionally lives alongside the frozen historical implementation.
//! `BaseRulesGame` remains the effect-free reference, while the separately re-exported
//! `ClanBonusDiagnostic` and `CombatStatDiagnosticV1` are explicit projected slices rather
//! than claims of a full-effects engine.

use crate::catalog::CardKey;
use std::error::Error;
use std::fmt;

mod catalog_match;
mod clan_bonus_diagnostic;
pub use catalog_match::*;
mod combat_resolution;
pub use clan_bonus_diagnostic::*;
pub(crate) mod combat_stat_compiler;
mod combat_stat_diagnostic;
pub use combat_stat_diagnostic::*;

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

/// A permanent effect that a round latched for one player. It applies at the end of every
/// later round for the rest of the match, whatever those rounds' outcomes, whichever card
/// its owner then plays, and even in the round its owner is knocked out (1073107 r3,
/// 1092294 r3). Whether the latching round itself pays is a property of the effect: the
/// TypeScript reference marks Heal and Poison `delayed` while Toxin and Regen pay at once,
/// and the server agrees - Lianah's Heal reports quantity 0 in its latch round (1091985 r1)
/// while Galactea's Toxin already takes one Life in hers (1091904 r1).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LatchedEffectV1 {
    /// `Heal N Max. M`: the owner gains N Life while below M, never past M, and only while
    /// still living. A player already at or above M is left exactly where they are.
    HealLife { life: u16, maximum: u16 },
    /// `Regen N, Max. M`: Heal that also pays in the latching round (1059149 r2: 5 to 6
    /// under Max 6, then 0 at the cap).
    RegenLife { life: u16, maximum: u16 },
    /// `Poison N, Min M`: the opposing player loses N Life while above M, never below M
    /// (1060510 r2 stays at 2 under Min 3), from the round after the latch.
    PoisonOpponentLife { life: u16, minimum: u16 },
    /// `Toxin N, Min M`: Poison that also pays in the latching round. With Min 0 it can end
    /// the match: 963039 r2 takes the opponent from 3 to 1 by Regan's reduction and then to
    /// 0 by the Toxin.
    ToxinOpponentLife { life: u16, minimum: u16 },
}

impl LatchedEffectV1 {
    /// Whether the round that latches this effect also pays it.
    pub const fn pays_in_latching_round(self) -> bool {
        match self {
            Self::HealLife { .. } | Self::PoisonOpponentLife { .. } => false,
            Self::RegenLife { .. } | Self::ToxinOpponentLife { .. } => true,
        }
    }
}

/// The permanents one player has latched so far, in latch order, which is also the order
/// they are applied in. Each card is played once and this projection latches at most one
/// source per card, so the hand size bounds the list.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct LatchedEffectsV1 {
    effects: [Option<LatchedEffectV1>; HAND_SIZE],
}

impl LatchedEffectsV1 {
    pub const EMPTY: Self = Self {
        effects: [None; HAND_SIZE],
    };

    pub fn iter(&self) -> impl Iterator<Item = LatchedEffectV1> + '_ {
        self.effects.iter().flatten().copied()
    }

    pub fn len(&self) -> usize {
        self.effects.iter().flatten().count()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.iter().all(Option::is_none)
    }

    /// Append in latch order. `Err` returns the effect when the list is already full, which
    /// a well-formed plan cannot reach.
    fn push(&mut self, effect: LatchedEffectV1) -> Result<(), LatchedEffectV1> {
        match self.effects.iter_mut().find(|slot| slot.is_none()) {
            Some(slot) => {
                *slot = Some(effect);
                Ok(())
            }
            None => Err(effect),
        }
    }
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
    /// Winner of the completed round immediately before this position, if any.
    /// This is part of the structural position so temporal predicates and future
    /// transposition keys cannot conflate otherwise identical states.
    pub previous_round_winner: Option<PlayerId>,
    /// The hand slot each player played in the completed round immediately before this
    /// position, if any. `After [clan:...]` reads its canonical clan.
    pub previous_round_slots: ByPlayer<Option<HandSlot>>,
    /// Permanents latched by completed rounds, per owner. They are mutable match state
    /// like Life: two positions that agree on everything else but differ here play out
    /// differently, so they belong to the structural position and its undo snapshot.
    pub latched: ByPlayer<LatchedEffectsV1>,
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
    PillzRecoveryOverflow {
        player: PlayerId,
    },
    PillzIncreaseOverflow {
        player: PlayerId,
    },
    LifeIncreaseOverflow {
        player: PlayerId,
    },
    /// More permanents latched for one player than the hand can play. Unreachable from a
    /// validated plan; kept as an error rather than a panic so the commit stays atomic.
    LatchedEffectOverflow {
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
            Self::PillzRecoveryOverflow { player } => {
                write!(formatter, "{player:?} pillz recovery overflow")
            }
            Self::PillzIncreaseOverflow { player } => {
                write!(formatter, "{player:?} pillz increase overflow")
            }
            Self::LifeIncreaseOverflow { player } => {
                write!(formatter, "{player:?} life increase overflow")
            }
            Self::LatchedEffectOverflow { player } => {
                write!(formatter, "{player:?} latched more permanents than cards")
            }
        }
    }
}

impl Error for BaseRulesError {}

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

/// Typed post-round work produced by an admitted diagnostic projection. Ordinary base rules
/// deliberately supply [`PostRoundPlan::default`], keeping effects outside their historical
/// combat preparation path.
#[derive(Clone, Copy, Default)]
pub(super) struct PostRoundPlan {
    pub ability: Option<PostRoundEffect>,
    pub bonus: Option<PostRoundEffect>,
}

/// Source-level post-round work before resolution has bound any selected-card-dependent
/// values. Fixed work stays explicit so old effects cannot accidentally acquire dynamic
/// semantics.
#[derive(Clone, Copy)]
pub(super) enum PostRoundSourceEffect {
    Fixed(PostRoundEffect),
    ReduceOpponentLifeOnVictoryPerOpponentStars {
        per_star: u16,
        minimum: u16,
    },
    /// The post-round `Brawl:` grammars. Each is an ordinary Victory effect whose printed
    /// amount is multiplied by the owner's anti-support count - the distinct characters in
    /// the opposing hand sharing the opposing selected card's effective clan - and binds to
    /// the fixed arm that already pays it. The clamp is applied once, after multiplying.
    ReduceOpponentLifeOnVictoryPerAntiSupport {
        per_count: u16,
        minimum: u16,
    },
    ReduceOpponentPillzOnVictoryPerAntiSupport {
        per_count: u16,
        minimum: u16,
    },
    /// `maximum == 0` is the uncapped `Brawl: +N Pillz`.
    GainPillzOnVictoryPerAntiSupport {
        per_count: u16,
        maximum: u16,
    },
    /// The post-round `Support:` grammars. The printed amount is multiplied by the owner's
    /// Support count - the distinct characters in its own hand sharing its selected card's
    /// effective clan, already on the source's resolution plan - and binds to the fixed arm
    /// that pays the plain grammar, clamped once after multiplying.
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
    /// card's stars, bound to the plain Victory gain.
    GainLifeOnVictoryPerOpponentStars {
        per_star: u16,
    },
    GainPillzOnVictoryPerOpponentStars {
        per_star: u16,
    },
    /// The `Growth:`/`Degrowth:` Victory grammars: the printed amount scaled by the round,
    /// bound to the fixed arm that pays the plain grammar.
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
}

/// Which end-of-round resource a post-round effect writes, for `Cancel Opp. ... Modif.`.
/// The compound, both-players and permanent kinds are the ones whose cancellation no
/// captured round has shown; construction refuses a canceller facing any of them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PostRoundResourceV1 {
    Life,
    Pillz,
    PillzAndLife,
    BothPlayersLife,
    Permanent,
}

impl PostRoundEffect {
    pub(super) const fn resource(self) -> PostRoundResourceV1 {
        match self {
            Self::RecoverPaidPillzOnDefeat
            | Self::GainOnePillzOnVictoryOrDefeat
            | Self::GainTwoPillzOnDefeatMaxEleven
            | Self::GainPillzOnVictory(_)
            | Self::GainPillzOnVictoryMax { .. }
            | Self::ReduceOpponentPillzOnVictory { .. }
            | Self::ReduceOpponentPillzOnDefeat { .. }
            | Self::GainPillzEqualToFinalDamageOnVictory
            | Self::GainPillzOnDefeat(_) => PostRoundResourceV1::Pillz,
            Self::GainLifeEqualToFinalDamageOnCourageVictory
            | Self::GainLifeOnVictory(_)
            | Self::GainLifePerFinalDamageOnVictory { .. }
            | Self::GainLifePerOpponentFinalDamageOnVictory { .. }
            | Self::GainLifeOnDefeat(_)
            | Self::ReanimateLife(_)
            | Self::GainLifeOnVictoryOrDefeat { .. }
            | Self::ReduceOpponentLifeOnVictoryOrDefeat { .. }
            | Self::ReduceOpponentLifeOnVictory { .. }
            | Self::ReduceOpponentLifeOnDefeat { .. }
            | Self::ReduceOpponentLifeOnKillshot { .. } => PostRoundResourceV1::Life,
            Self::GainOnePillzAndLifeOnVictory
            | Self::GainPillzAndLifeOnKillshot { .. }
            | Self::GainPillzAndLifeOnDefeat(_) => PostRoundResourceV1::PillzAndLife,
            Self::ReduceBothPlayersLife { .. } => PostRoundResourceV1::BothPlayersLife,
            Self::LatchOnVictory(_) | Self::LatchOnDefeat(_) => PostRoundResourceV1::Permanent,
        }
    }
}

impl PostRoundSourceEffect {
    pub(super) const fn resource(self) -> PostRoundResourceV1 {
        match self {
            Self::Fixed(effect) => effect.resource(),
            Self::ReduceOpponentLifeOnVictoryPerOpponentStars { .. }
            | Self::ReduceOpponentLifeOnVictoryPerAntiSupport { .. }
            | Self::ReduceOpponentLifeOnVictoryPerRound { .. }
            | Self::GainLifeOnVictoryPerRound { .. }
            | Self::ReduceOpponentLifeOnVictoryPerSupport { .. }
            | Self::GainLifeOnVictoryPerSupport { .. }
            | Self::GainLifeOnVictoryPerOpponentStars { .. } => PostRoundResourceV1::Life,
            Self::ReduceOpponentPillzOnVictoryPerAntiSupport { .. }
            | Self::GainPillzOnVictoryPerAntiSupport { .. }
            | Self::ReduceOpponentPillzOnVictoryPerRound { .. }
            | Self::GainPillzOnVictoryPerRound { .. }
            | Self::GainPillzOnVictoryPerSupport { .. }
            | Self::GainPillzOnVictoryPerOpponentStars { .. } => PostRoundResourceV1::Pillz,
        }
    }

    pub(super) const fn life_beneficiary(self) -> LifeBeneficiaryV1 {
        match self {
            Self::Fixed(effect) => effect.life_beneficiary(),
            Self::GainLifeOnVictoryPerRound { .. }
            | Self::GainLifeOnVictoryPerSupport { .. }
            | Self::GainLifeOnVictoryPerOpponentStars { .. } => LifeBeneficiaryV1::Owner,
            Self::ReduceOpponentLifeOnVictoryPerOpponentStars { .. }
            | Self::ReduceOpponentLifeOnVictoryPerAntiSupport { .. }
            | Self::ReduceOpponentLifeOnVictoryPerRound { .. }
            | Self::ReduceOpponentLifeOnVictoryPerSupport { .. }
            | Self::ReduceOpponentPillzOnVictoryPerAntiSupport { .. }
            | Self::GainPillzOnVictoryPerAntiSupport { .. }
            | Self::ReduceOpponentPillzOnVictoryPerRound { .. }
            | Self::GainPillzOnVictoryPerRound { .. }
            | Self::GainPillzOnVictoryPerSupport { .. }
            | Self::GainPillzOnVictoryPerOpponentStars { .. } => LifeBeneficiaryV1::Nobody,
        }
    }
}

impl PostRoundSourceEffect {
    /// Whether the effect's trigger reads both final Attacks, which only Killshot does.
    pub(super) const fn reads_final_attacks(self) -> bool {
        match self {
            Self::Fixed(effect) => effect.reads_final_attacks(),
            Self::ReduceOpponentLifeOnVictoryPerOpponentStars { .. }
            | Self::ReduceOpponentLifeOnVictoryPerAntiSupport { .. }
            | Self::ReduceOpponentLifeOnVictoryPerRound { .. }
            | Self::GainLifeOnVictoryPerRound { .. }
            | Self::ReduceOpponentPillzOnVictoryPerAntiSupport { .. }
            | Self::GainPillzOnVictoryPerAntiSupport { .. }
            | Self::ReduceOpponentPillzOnVictoryPerRound { .. }
            | Self::GainPillzOnVictoryPerRound { .. }
            | Self::ReduceOpponentLifeOnVictoryPerSupport { .. }
            | Self::GainLifeOnVictoryPerSupport { .. }
            | Self::GainPillzOnVictoryPerSupport { .. }
            | Self::GainLifeOnVictoryPerOpponentStars { .. }
            | Self::GainPillzOnVictoryPerOpponentStars { .. } => false,
        }
    }
}

impl PostRoundEffect {
    pub(super) const fn reads_final_attacks(self) -> bool {
        match self {
            Self::ReduceOpponentLifeOnKillshot { .. } | Self::GainPillzAndLifeOnKillshot { .. } => {
                true
            }
            Self::RecoverPaidPillzOnDefeat
            | Self::GainOnePillzOnVictoryOrDefeat
            | Self::GainOnePillzAndLifeOnVictory
            | Self::GainTwoPillzOnDefeatMaxEleven
            | Self::GainLifeEqualToFinalDamageOnCourageVictory
            | Self::GainLifeOnVictory(_)
            | Self::GainPillzOnVictory(_)
            | Self::GainPillzOnVictoryMax { .. }
            | Self::ReduceOpponentPillzOnVictory { .. }
            | Self::ReduceOpponentPillzOnDefeat { .. }
            | Self::GainPillzEqualToFinalDamageOnVictory
            | Self::GainLifePerFinalDamageOnVictory { .. }
            | Self::GainLifePerOpponentFinalDamageOnVictory { .. }
            | Self::GainLifeOnDefeat(_)
            | Self::ReanimateLife(_)
            | Self::GainLifeOnVictoryOrDefeat { .. }
            | Self::ReduceOpponentLifeOnVictoryOrDefeat { .. }
            | Self::ReduceOpponentLifeOnVictory { .. }
            | Self::ReduceOpponentLifeOnDefeat { .. }
            | Self::ReduceBothPlayersLife { .. }
            | Self::LatchOnVictory(_)
            | Self::LatchOnDefeat(_)
            | Self::GainPillzOnDefeat(_)
            | Self::GainPillzAndLifeOnDefeat(_) => false,
        }
    }
}

/// Whose Life a post-round effect can raise. `/ Life Lost` is pinned only for an owner
/// whose Life has never risen during the match, so construction refuses one wherever
/// anything could raise it; the match is exhaustive so that a new effect has to say.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LifeBeneficiaryV1 {
    Nobody,
    Owner,
}

impl PostRoundEffect {
    pub(super) const fn life_beneficiary(self) -> LifeBeneficiaryV1 {
        match self {
            Self::GainOnePillzAndLifeOnVictory
            | Self::GainLifeEqualToFinalDamageOnCourageVictory
            | Self::GainLifeOnVictory(_)
            | Self::GainLifePerFinalDamageOnVictory { .. }
            | Self::GainLifePerOpponentFinalDamageOnVictory { .. }
            | Self::GainLifeOnDefeat(_)
            | Self::ReanimateLife(_)
            | Self::GainLifeOnVictoryOrDefeat { .. }
            | Self::GainPillzAndLifeOnKillshot { .. }
            | Self::GainPillzAndLifeOnDefeat(_)
            | Self::LatchOnVictory(
                LatchedEffectV1::HealLife { .. } | LatchedEffectV1::RegenLife { .. },
            )
            | Self::LatchOnDefeat(
                LatchedEffectV1::HealLife { .. } | LatchedEffectV1::RegenLife { .. },
            ) => LifeBeneficiaryV1::Owner,
            Self::RecoverPaidPillzOnDefeat
            | Self::GainOnePillzOnVictoryOrDefeat
            | Self::GainTwoPillzOnDefeatMaxEleven
            | Self::GainPillzOnVictory(_)
            | Self::GainPillzOnVictoryMax { .. }
            | Self::ReduceOpponentPillzOnVictory { .. }
            | Self::ReduceOpponentPillzOnDefeat { .. }
            | Self::GainPillzEqualToFinalDamageOnVictory
            | Self::GainPillzOnDefeat(_)
            | Self::ReduceOpponentLifeOnVictoryOrDefeat { .. }
            | Self::ReduceOpponentLifeOnVictory { .. }
            | Self::ReduceOpponentLifeOnDefeat { .. }
            | Self::ReduceOpponentLifeOnKillshot { .. }
            | Self::ReduceBothPlayersLife { .. }
            | Self::LatchOnVictory(
                LatchedEffectV1::PoisonOpponentLife { .. }
                | LatchedEffectV1::ToxinOpponentLife { .. },
            )
            | Self::LatchOnDefeat(
                LatchedEffectV1::PoisonOpponentLife { .. }
                | LatchedEffectV1::ToxinOpponentLife { .. },
            ) => LifeBeneficiaryV1::Nobody,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum PostRoundEffect {
    RecoverPaidPillzOnDefeat,
    GainOnePillzOnVictoryOrDefeat,
    GainOnePillzAndLifeOnVictory,
    GainTwoPillzOnDefeatMaxEleven,
    /// Anita's exact Courage conversion. Its magnitude is the selected winner's final
    /// resolved damage, so Fury and any already-resolved combat damage modifiers count.
    GainLifeEqualToFinalDamageOnCourageVictory,
    GainLifeOnVictory(u16),
    /// Plain `+N Pillz`: the round winner's own Pillz rise by the printed amount.
    GainPillzOnVictory(u16),
    /// The capped own gain `Brawl: +N Pillz, Max. M` binds to: the winner's own Pillz rise by
    /// `pillz`, never past `maximum`, and an owner already at or above it gains nothing.
    GainPillzOnVictoryMax {
        pillz: u16,
        maximum: u16,
    },
    /// Plain `-N Opp Pillz. Min M`: the round winner takes `pillz` from the opposing
    /// player's remaining Pillz, never below `minimum`.
    ReduceOpponentPillzOnVictory {
        pillz: u16,
        minimum: u16,
    },
    /// `Defeat: -N Opp. Pillz, Min M`: the losing owner takes `pillz` from the opposing
    /// player's remaining Pillz, never below `minimum`.
    ReduceOpponentPillzOnDefeat {
        pillz: u16,
        minimum: u16,
    },
    /// `+1 Pillz Per Damage`: the winner's own Pillz rise by the final resolved Damage its
    /// card dealt, Fury and combat modifiers included.
    GainPillzEqualToFinalDamageOnVictory,
    /// `+N Life Per Damage`: the winner's own Life rises by N for every point of that same
    /// final resolved Damage. A capped `Max. M` record never carries its owner past M and
    /// pays nothing to an owner already there, exactly as `Heal N Max. M` does on the
    /// latch; `maximum` 0 is the uncapped form.
    GainLifePerFinalDamageOnVictory {
        life_per_damage: u16,
        maximum: u16,
    },
    /// `+N Life Per Opp. Damage`: the winner's own Life rises by `life_per_damage` for
    /// every point of the *losing* card's final resolved Damage, Fury included.
    GainLifePerOpponentFinalDamageOnVictory {
        life_per_damage: u16,
    },
    GainLifeOnDefeat(u16),
    ReanimateLife(u16),
    GainLifeOnVictoryOrDefeat {
        life: u16,
    },
    ReduceOpponentLifeOnVictoryOrDefeat {
        life: u16,
        minimum: u16,
    },
    ReduceOpponentLifeOnVictory {
        life: u16,
        minimum: u16,
    },
    ReduceOpponentLifeOnDefeat {
        life: u16,
        minimum: u16,
    },
    /// `Killshot: -N Opp. Life Min M`: paid when the owner's final attack is at least
    /// double the opposing one, which the reference evaluates without reference to the
    /// round's winner.
    ReduceOpponentLifeOnKillshot {
        life: u16,
        minimum: u16,
    },
    /// `Xantiax: -N Life, Min. M`: both players lose `life`, neither below `minimum`,
    /// whatever the round's outcome and whichever side owns the source.
    ReduceBothPlayersLife {
        life: u16,
        minimum: u16,
    },
    /// A permanent on a live source whose owner wins this round: latch it into the owner's
    /// position so every later round pays it. Whether this round pays it too is the
    /// effect's own property.
    LatchOnVictory(LatchedEffectV1),
    /// `Defeat: Poison N, Min M`. The same latch on the losing side: the owner having lost
    /// the round is the trigger. Once latched it is indistinguishable from any other
    /// permanent, so the repeat loop needs no knowledge of how it got there.
    LatchOnDefeat(LatchedEffectV1),
    /// `Killshot: +N Pillz And Life`: the owner's final attack at least doubling the opposing
    /// one gives a living owner N Pillz and then N Life.
    GainPillzAndLifeOnKillshot {
        amount: u16,
    },
    /// `Defeat: +N Pillz`: a living loser's own Pillz rise by N.
    GainPillzOnDefeat(u16),
    /// `Defeat: +N Pillz And Life`: a living loser gains N Pillz and then N Life.
    GainPillzAndLifeOnDefeat(u16),
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
                previous_round_winner: None,
                previous_round_slots: ByPlayer::new(None, None),
                latched: ByPlayer::new(LatchedEffectsV1::EMPTY, LatchedEffectsV1::EMPTY),
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
        self.commit(
            input,
            prepared,
            ByPlayer::new(PostRoundPlan::default(), PostRoundPlan::default()),
        )
    }

    pub fn unmake(&mut self, undo: BaseRulesUndo) {
        self.position = undo.before;
    }

    fn commit(
        &mut self,
        input: BaseRulesRoundInput,
        prepared: ByPlayer<PreparedSelection>,
        post_round: ByPlayer<PostRoundPlan>,
    ) -> Result<(BaseRulesRoundReport, BaseRulesUndo), BaseRulesError> {
        // Work on a complete replacement position so a post-round overflow cannot leave a
        // partially committed card, cost, or winner behind. On success, move the original
        // position directly into the undo token instead of cloning this hot-path state twice.
        let mut position = self.position.clone();
        let round = position.rounds_played;

        for player in PlayerId::ALL {
            let selected = prepared[player];
            position.players[player].pillz -= selected.cost;
            position.played[player][selected.slot.index()] = true;
        }

        let winner = round_winner(input.first_mover, &prepared);
        let loser = winner.other();
        position.players[loser].life = position.players[loser]
            .life
            .saturating_sub(prepared[winner].result.damage);
        // Post-round effects belong to their selected owner. Defeat recovery remains
        // deliberately loser-only; Victory Or Defeat applies after the costs, winner, and
        // damage for either owner, including a KO.
        for owner in PlayerId::ALL {
            // Match the TypeScript reference's within-phase ordering: the clan bonus is
            // registered before the ability. The VOD effects commute, but Argos' capped
            // post-round effect makes this ordering observable.
            for effect in [post_round[owner].bonus, post_round[owner].ability]
                .into_iter()
                .flatten()
            {
                match effect {
                    PostRoundEffect::RecoverPaidPillzOnDefeat if owner == loser => {
                        let paid = u32::from(prepared[owner].cost);
                        let recovered = ((paid * 2 + 2) / 3).max(1) as u16;
                        position.players[owner].pillz = position.players[owner]
                            .pillz
                            .checked_add(recovered)
                            .ok_or(BaseRulesError::PillzRecoveryOverflow { player: owner })?;
                    }
                    PostRoundEffect::RecoverPaidPillzOnDefeat => {}
                    PostRoundEffect::GainOnePillzOnVictoryOrDefeat => {
                        position.players[owner].pillz = position.players[owner]
                            .pillz
                            .checked_add(1)
                            .ok_or(BaseRulesError::PillzIncreaseOverflow { player: owner })?;
                    }
                    // Komboka's Bonus-only composite Victory effect pays a living winner,
                    // including after the opponent is KO'd.  Keep its checked mutations in
                    // TypeScript order (Pillz, then Life); `position` is still a private
                    // replacement, so a second-step overflow remains fully atomic.
                    PostRoundEffect::GainOnePillzAndLifeOnVictory
                        if owner == winner && position.players[owner].life > 0 =>
                    {
                        position.players[owner].pillz = position.players[owner]
                            .pillz
                            .checked_add(1)
                            .ok_or(BaseRulesError::PillzIncreaseOverflow { player: owner })?;
                        position.players[owner].life = position.players[owner]
                            .life
                            .checked_add(1)
                            .ok_or(BaseRulesError::LifeIncreaseOverflow { player: owner })?;
                    }
                    PostRoundEffect::GainOnePillzAndLifeOnVictory => {}
                    // Argos is ordinary post-round Pillz work: unlike the audited VOD
                    // sources, it only pays on a surviving defeat. Its Max belongs to this
                    // modifier, after the clan bonus has already run; never lower a value
                    // that the earlier bonus raised to or above eleven.
                    PostRoundEffect::GainTwoPillzOnDefeatMaxEleven
                        if owner == loser && position.players[owner].life > 0 =>
                    {
                        let pillz = position.players[owner].pillz;
                        if pillz < 11 {
                            position.players[owner].pillz = pillz
                                .checked_add(2)
                                .ok_or(BaseRulesError::PillzIncreaseOverflow { player: owner })?
                                .min(11);
                        }
                    }
                    PostRoundEffect::GainTwoPillzOnDefeatMaxEleven => {}
                    // Anita's reviewed conversion is ordinary, post-damage Victory Life:
                    // its owner must have moved first, won, and still be living. The
                    // selected result is final damage (after Fury and combat modifiers),
                    // not the printed base stat. A terminal zero is deliberately never
                    // revived without separate capture evidence.
                    PostRoundEffect::GainLifeEqualToFinalDamageOnCourageVictory
                        if owner == winner
                            && owner == input.first_mover
                            && position.players[owner].life > 0 =>
                    {
                        position.players[owner].life = position.players[owner]
                            .life
                            .checked_add(prepared[owner].result.damage)
                            .ok_or(BaseRulesError::LifeIncreaseOverflow { player: owner })?;
                    }
                    PostRoundEffect::GainLifeEqualToFinalDamageOnCourageVictory => {}
                    // Victory Life is immediate end-of-round work: it sees the damage
                    // result, applies only to the round winner, and can therefore revive
                    // neither a defeated player nor a KO.  `position` is still a private
                    // replacement until every checked addition succeeds, keeping failure
                    // atomic and make/unmake exact.
                    PostRoundEffect::GainLifeOnVictory(life) if owner == winner => {
                        position.players[owner].life = position.players[owner]
                            .life
                            .checked_add(life)
                            .ok_or(BaseRulesError::LifeIncreaseOverflow { player: owner })?;
                    }
                    PostRoundEffect::GainLifeOnVictory(_) => {}
                    // Plain Victory Pillz is the same immediate winner-only work on the
                    // other resource. Like every ordinary own gain it pays a living owner
                    // only: the TypeScript reference skips a Pillz gain for a player at
                    // zero, which an earlier owner's repeating Toxin can produce. The
                    // opponent being knocked out changes nothing (capture 1092454/3).
                    PostRoundEffect::GainPillzOnVictory(pillz)
                        if owner == winner && position.players[owner].life > 0 =>
                    {
                        position.players[owner].pillz = position.players[owner]
                            .pillz
                            .checked_add(pillz)
                            .ok_or(BaseRulesError::PillzIncreaseOverflow { player: owner })?;
                    }
                    PostRoundEffect::GainPillzOnVictory(_) => {}
                    // The capped form is the same living-winner gain with Argos' cap
                    // arithmetic: the TypeScript modifier leaves a value already at or above
                    // Max alone and otherwise clamps once, after the multiplied amount.
                    PostRoundEffect::GainPillzOnVictoryMax { pillz, maximum }
                        if owner == winner && position.players[owner].life > 0 =>
                    {
                        let current = position.players[owner].pillz;
                        if current < maximum {
                            position.players[owner].pillz = current
                                .checked_add(pillz)
                                .ok_or(BaseRulesError::PillzIncreaseOverflow { player: owner })?
                                .min(maximum);
                        }
                    }
                    PostRoundEffect::GainPillzOnVictoryMax { .. } => {}
                    // The opposing reduction is Victory-only and reads the target's Pillz
                    // after both bets have been paid, which is where the TypeScript
                    // reference applies END modifiers. A target already at or below Min is
                    // left alone rather than pulled up to it (1091644/1: AI-Lycs recovers to
                    // 4 under Min 4 and Dalhia Cr's -3 changes nothing).
                    PostRoundEffect::ReduceOpponentPillzOnVictory { pillz, minimum }
                        if owner == winner && position.players[owner.other()].pillz > minimum =>
                    {
                        let target = owner.other();
                        position.players[target].pillz = position.players[target]
                            .pillz
                            .saturating_sub(pillz)
                            .max(minimum);
                    }
                    PostRoundEffect::ReduceOpponentPillzOnVictory { .. } => {}
                    // The losing-side reduction composes two already-pinned pieces: the
                    // Victory reduction's arithmetic - read after both bets, leave a target
                    // at or below Min alone - and the Defeat channel's trigger, which the
                    // reviewed opponent-Life sibling establishes pays out even from an owner
                    // this round has knocked out. 1092515/0 and 1092201/2 pin the arithmetic
                    // away from the floor; the corpus has no knocked-out owner carrying it,
                    // so that half is the sibling's evidence, not its own.
                    PostRoundEffect::ReduceOpponentPillzOnDefeat { pillz, minimum }
                        if owner != winner && position.players[owner.other()].pillz > minimum =>
                    {
                        let target = owner.other();
                        position.players[target].pillz = position.players[target]
                            .pillz
                            .saturating_sub(pillz)
                            .max(minimum);
                    }
                    PostRoundEffect::ReduceOpponentPillzOnDefeat { .. } => {}
                    // The conversion reads the same final damage Anita's does - the
                    // TypeScript multiplier is `card.damage.final` - and pays a living
                    // winner. Its Symmetry form is a predicate on the plan, judged before
                    // this effect is ever handed to the engine.
                    PostRoundEffect::GainPillzEqualToFinalDamageOnVictory
                        if owner == winner && position.players[owner].life > 0 =>
                    {
                        position.players[owner].pillz = position.players[owner]
                            .pillz
                            .checked_add(prepared[owner].result.damage)
                            .ok_or(BaseRulesError::PillzIncreaseOverflow { player: owner })?;
                    }
                    PostRoundEffect::GainPillzEqualToFinalDamageOnVictory => {}
                    // The Life conversion is Anita's arithmetic without her Courage: the
                    // winner gains N per point of final damage while living, and the
                    // opponent's knockout changes nothing (1089830/2: 12 + 4 while the
                    // target falls to zero). Its Revenge and Confidence forms are plan
                    // predicates judged before the effect is handed here.
                    PostRoundEffect::GainLifePerFinalDamageOnVictory {
                        life_per_damage,
                        maximum,
                    } if owner == winner && position.players[owner].life > 0 => {
                        let current = position.players[owner].life;
                        // The cap is Heal's, read at the moment this effect pays: an owner
                        // already at or past it gains nothing, and a conversion that would
                        // overshoot stops exactly there (1130609/3: 5 + 6 reaches 8, not 11).
                        if maximum == 0 || current < maximum {
                            let gain = prepared[owner]
                                .result
                                .damage
                                .checked_mul(life_per_damage)
                                .ok_or(BaseRulesError::LifeIncreaseOverflow {
                                player: owner,
                            })?;
                            let raised = current
                                .checked_add(gain)
                                .ok_or(BaseRulesError::LifeIncreaseOverflow { player: owner })?;
                            position.players[owner].life = if maximum == 0 {
                                raised
                            } else {
                                raised.min(maximum)
                            };
                        }
                    }
                    PostRoundEffect::GainLifePerFinalDamageOnVictory { .. } => {}
                    PostRoundEffect::GainLifePerOpponentFinalDamageOnVictory {
                        life_per_damage,
                    } if owner == winner && position.players[owner].life > 0 => {
                        let gain = prepared[owner.other()]
                            .result
                            .damage
                            .checked_mul(life_per_damage)
                            .ok_or(BaseRulesError::LifeIncreaseOverflow { player: owner })?;
                        position.players[owner].life = position.players[owner]
                            .life
                            .checked_add(gain)
                            .ok_or(BaseRulesError::LifeIncreaseOverflow { player: owner })?;
                    }
                    PostRoundEffect::GainLifePerOpponentFinalDamageOnVictory { .. } => {}
                    // The reviewed Victory Or Defeat Life sources are ordinary Life
                    // gains, not Reanimate: a living owner gains after either outcome,
                    // while a KO remains terminal.  Keep the addition checked so the
                    // replacement-position commit retains its all-or-nothing guarantee.
                    PostRoundEffect::GainLifeOnVictoryOrDefeat { life }
                        if position.players[owner].life > 0 =>
                    {
                        position.players[owner].life = position.players[owner]
                            .life
                            .checked_add(life)
                            .ok_or(BaseRulesError::LifeIncreaseOverflow { player: owner })?;
                    }
                    PostRoundEffect::GainLifeOnVictoryOrDefeat { .. } => {}
                    // Uuber's reviewed Victory Or Defeat reduction is directed at the
                    // opposing player.  It still happens when its owner was KO'd, but a
                    // target already at zero must never be revived by the Min clamp.
                    PostRoundEffect::ReduceOpponentLifeOnVictoryOrDefeat { life, minimum } => {
                        let target = owner.other();
                        if position.players[target].life > 0 {
                            position.players[target].life = position.players[target]
                                .life
                                .saturating_sub(life)
                                .max(minimum);
                        }
                    }
                    // Equalizer's magnitude was bound from the revealed opposing card
                    // after Stop liveness. It is Victory-only: neither an owner KO nor a
                    // defeated owner may affect the opponent, and a target at/below Min
                    // must remain untouched.
                    PostRoundEffect::ReduceOpponentLifeOnVictory { life, minimum }
                        if owner == winner && position.players[owner.other()].life > minimum =>
                    {
                        let target = owner.other();
                        position.players[target].life = position.players[target]
                            .life
                            .saturating_sub(life)
                            .max(minimum);
                    }
                    PostRoundEffect::ReduceOpponentLifeOnVictory { .. } => {}
                    // The losing-side reduction mirrors it: the owner having lost is the
                    // trigger, so an owner taken to zero still pays it out, exactly as the
                    // reviewed Victory Or Defeat reduction does, and a target already at or
                    // below Min is left untouched rather than pulled up to it.
                    PostRoundEffect::ReduceOpponentLifeOnDefeat { life, minimum }
                        if owner != winner && position.players[owner.other()].life > minimum =>
                    {
                        let target = owner.other();
                        position.players[target].life = position.players[target]
                            .life
                            .saturating_sub(life)
                            .max(minimum);
                    }
                    PostRoundEffect::ReduceOpponentLifeOnDefeat { .. } => {}
                    // Killshot asks the attack ratio, not the winner. The reference
                    // (`Condition::Killshot` in `ability.rs`) is `attack >= opp_attack * 2`
                    // with no win requirement, unlike its `Backlash` neighbour, so this
                    // must not be written as `owner == winner && ratio`: at equal attacks -
                    // reachable at zero through a `Min 0` Power reduction - the ratio holds
                    // while `round_winner` may hand the round to the other side. Both final
                    // attacks are already resolved in `prepared`, so the trigger reads them
                    // directly. Widen before doubling so a large attack cannot wrap. A
                    // target already at or below Min is left untouched rather than pulled
                    // up to it, exactly as the Victory and Defeat siblings do.
                    PostRoundEffect::ReduceOpponentLifeOnKillshot { life, minimum }
                        if u64::from(prepared[owner].result.attack)
                            >= 2 * u64::from(prepared[owner.other()].result.attack)
                            && position.players[owner.other()].life > minimum =>
                    {
                        let target = owner.other();
                        position.players[target].life = position.players[target]
                            .life
                            .saturating_sub(life)
                            .max(minimum);
                    }
                    PostRoundEffect::ReduceOpponentLifeOnKillshot { .. } => {}
                    // The Killshot compound is the Komboka pair of own gains on the Killshot
                    // trigger above: the attack ratio, not the winner, and a living owner.
                    // Pillz then Life, each checked, as Komboka pays them.
                    PostRoundEffect::GainPillzAndLifeOnKillshot { amount }
                        if u64::from(prepared[owner].result.attack)
                            >= 2 * u64::from(prepared[owner.other()].result.attack)
                            && position.players[owner].life > 0 =>
                    {
                        position.players[owner].pillz = position.players[owner]
                            .pillz
                            .checked_add(amount)
                            .ok_or(BaseRulesError::PillzIncreaseOverflow { player: owner })?;
                        position.players[owner].life = position.players[owner]
                            .life
                            .checked_add(amount)
                            .ok_or(BaseRulesError::LifeIncreaseOverflow { player: owner })?;
                    }
                    PostRoundEffect::GainPillzAndLifeOnKillshot { .. } => {}
                    // Xantiax is the only admitted post-round effect with no outcome
                    // channel and no beneficiary: it takes from both players at once. The
                    // owner winning, losing or being knocked out by this round's damage
                    // makes no difference (1058151/3 pays into the opponent from an owner
                    // the round has just taken to zero; 1080464/2 charges a winning owner),
                    // and neither player may be revived from zero or pulled up to Min by
                    // the clamp.
                    PostRoundEffect::ReduceBothPlayersLife { life, minimum } => {
                        for player in [owner, owner.other()] {
                            if position.players[player].life > minimum {
                                position.players[player].life = position.players[player]
                                    .life
                                    .saturating_sub(life)
                                    .max(minimum);
                            }
                        }
                    }
                    // Ordinary Defeat Life is post-damage work for a loss that did not KO
                    // its owner. It deliberately cannot turn a terminal zero back into a
                    // live position.
                    PostRoundEffect::GainLifeOnDefeat(life)
                        if owner == loser && position.players[owner].life > 0 =>
                    {
                        position.players[owner].life = position.players[owner]
                            .life
                            .checked_add(life)
                            .ok_or(BaseRulesError::LifeIncreaseOverflow { player: owner })?;
                    }
                    PostRoundEffect::GainLifeOnDefeat(_) => {}
                    // The Defeat own gains follow Defeat Life: a loser this round has not
                    // knocked out. Kubra's knockouts in 876712/1 and 877023/1 pay neither half
                    // of the compound, and it pays Pillz then Life, as Komboka does.
                    PostRoundEffect::GainPillzOnDefeat(pillz)
                        if owner == loser && position.players[owner].life > 0 =>
                    {
                        position.players[owner].pillz = position.players[owner]
                            .pillz
                            .checked_add(pillz)
                            .ok_or(BaseRulesError::PillzIncreaseOverflow { player: owner })?;
                    }
                    PostRoundEffect::GainPillzOnDefeat(_) => {}
                    PostRoundEffect::GainPillzAndLifeOnDefeat(amount)
                        if owner == loser && position.players[owner].life > 0 =>
                    {
                        position.players[owner].pillz = position.players[owner]
                            .pillz
                            .checked_add(amount)
                            .ok_or(BaseRulesError::PillzIncreaseOverflow { player: owner })?;
                        position.players[owner].life = position.players[owner]
                            .life
                            .checked_add(amount)
                            .ok_or(BaseRulesError::LifeIncreaseOverflow { player: owner })?;
                    }
                    PostRoundEffect::GainPillzAndLifeOnDefeat(_) => {}
                    // Reanimate is the explicit Life exception: damage has already been
                    // saturated at zero, and revival happens before status is calculated.
                    PostRoundEffect::ReanimateLife(life) if owner == loser => {
                        position.players[owner].life = position.players[owner]
                            .life
                            .checked_add(life)
                            .ok_or(BaseRulesError::LifeIncreaseOverflow { player: owner })?;
                    }
                    PostRoundEffect::ReanimateLife(_) => {}
                    // A permanent latches in the round its card wins. From here on it
                    // belongs to the owner, not to the card, and the repeat loop below
                    // pays it after every later round - and after this one, if it is the
                    // kind that pays at once.
                    PostRoundEffect::LatchOnVictory(effect) if owner == winner => {
                        position.latched[owner]
                            .push(effect)
                            .map_err(|_| BaseRulesError::LatchedEffectOverflow { player: owner })?;
                    }
                    PostRoundEffect::LatchOnVictory(_) => {}
                    // The losing-side latch mirrors it. An owner taken to zero by the round
                    // it lost still latches, exactly as the reviewed Defeat opponent-Life
                    // reduction still pays: the repeat loop's own guards decide from there.
                    PostRoundEffect::LatchOnDefeat(effect) if owner == loser => {
                        position.latched[owner]
                            .push(effect)
                            .map_err(|_| BaseRulesError::LatchedEffectOverflow { player: owner })?;
                    }
                    PostRoundEffect::LatchOnDefeat(_) => {}
                }
            }
            // Latched permanents repeat now, after this owner's own current-round effects
            // and before the other owner's, which is the TypeScript reference's END order:
            // each side's fresh effects, then its `repeat` bucket in latch order. Anything
            // latched just above sits past the pre-round length and pays this round only if
            // its kind does. A KO is terminal for the owner's own gains - a player at zero
            // is never revived - while an opposing reduction still lands on a living target
            // whether or not its owner was just knocked out.
            let latched_before = self.position.latched[owner].len();
            let latched = position.latched[owner];
            for (index, effect) in latched.iter().enumerate() {
                if index >= latched_before && !effect.pays_in_latching_round() {
                    continue;
                }
                match effect {
                    LatchedEffectV1::HealLife { life, maximum }
                    | LatchedEffectV1::RegenLife { life, maximum } => {
                        let current = position.players[owner].life;
                        if current > 0 && current < maximum {
                            position.players[owner].life = current
                                .checked_add(life)
                                .ok_or(BaseRulesError::LifeIncreaseOverflow { player: owner })?
                                .min(maximum);
                        }
                    }
                    LatchedEffectV1::PoisonOpponentLife { life, minimum }
                    | LatchedEffectV1::ToxinOpponentLife { life, minimum } => {
                        let target = owner.other();
                        let current = position.players[target].life;
                        if current > minimum {
                            position.players[target].life =
                                current.saturating_sub(life).max(minimum);
                        }
                    }
                }
            }
        }
        position.rounds_played += 1;
        position.previous_round_winner = Some(winner);
        position.previous_round_slots = ByPlayer::new(
            Some(prepared[PlayerId::P1].slot),
            Some(prepared[PlayerId::P2].slot),
        );
        position.status = status_after_round(&position);
        let undo = BaseRulesUndo {
            before: std::mem::replace(&mut self.position, position),
        };

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
        Ok((report, undo))
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
