//! Replay execution for the current engine's effects-disabled base-rules slice.
//!
//! Expected capture outputs are assertion targets only. Match construction and every round
//! input use source identity, canonical card data, and submitted selections exclusively.

use super::model::{
    EnginePlayer, ExpectedCardResult, ReplayCaseV1, ReplayPlay, ReplayRound, REPLAY_SCHEMA_VERSION,
};
use crate::catalog::{CardCatalog, CardKey};
use crate::engine::{
    BaseRulesCardResult, BaseRulesCardSpec, BaseRulesError, BaseRulesGame, BaseRulesMatchSpec,
    BaseRulesPlayerSpec, BaseRulesPosition, BaseRulesRoundInput, BaseRulesRoundReport,
    BaseRulesSelection, ByPlayer, PlayerId, HAND_SIZE, MAX_ROUNDS,
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

pub(super) fn round_input(
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
