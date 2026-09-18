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
    ReduceOpponentLifeOnVictoryPerOpponentStars { per_star: u16, minimum: u16 },
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
    /// Plain `-N Opp Pillz. Min M`: the round winner takes `pillz` from the opposing
    /// player's remaining Pillz, never below `minimum`.
    ReduceOpponentPillzOnVictory {
        pillz: u16,
        minimum: u16,
    },
    /// `+1 Pillz Per Damage`: the winner's own Pillz rise by the final resolved Damage its
    /// card dealt, Fury and combat modifiers included.
    GainPillzEqualToFinalDamageOnVictory,
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
    /// A permanent on a live source whose owner wins this round: latch it into the owner's
    /// position so every later round pays it. Whether this round pays it too is the
    /// effect's own property.
    LatchOnVictory(LatchedEffectV1),
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
