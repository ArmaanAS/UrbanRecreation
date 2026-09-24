use super::capture::{
    CapturedCard, CapturedGame, CapturedModifier, CapturedPlayer, CapturedResolution,
    CapturedTestcase, CapturedTestcaseResult,
};
use super::model::{
    EnginePlayer, ExpectedCardResult, ExpectedPlayerState, ReplayCard, ReplayCaseV1,
    ReplayMetadata, ReplayPlay, ReplayPlayer, ReplayRound, SourceClub, SourceModifier,
    SourceProfile, SourceRoom, SourceSide, SourceStatus, REPLAY_SCHEMA_VERSION,
};
use crate::catalog::{CardCatalog, CardKey};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplaySkipReason {
    NoReplayTestcase,
    InProgress,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SkippedReplay {
    pub battle_id: u64,
    pub source_status: String,
    pub reason: ReplaySkipReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplayClassification {
    Ready(Box<ReplayCaseV1>),
    Skipped(SkippedReplay),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterError {
    pub battle_id: u64,
    pub kind: AdapterErrorKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterErrorKind {
    InvalidField {
        field: String,
        detail: String,
    },
    UnknownCard {
        source_side: SourceSide,
        hand_index: u8,
        key: CardKey,
    },
    TestcaseMismatch {
        field: String,
        capture: String,
        testcase: String,
    },
}

impl fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "battle {}: ", self.battle_id)?;
        match &self.kind {
            AdapterErrorKind::InvalidField { field, detail } => {
                write!(formatter, "invalid {field}: {detail}")
            }
            AdapterErrorKind::UnknownCard {
                source_side,
                hand_index,
                key,
            } => write!(
                formatter,
                "unknown card {} level {} for {source_side:?} hand index {hand_index}",
                key.id, key.level
            ),
            AdapterErrorKind::TestcaseMismatch {
                field,
                capture,
                testcase,
            } => write!(
                formatter,
                "testcase mismatch at {field}: capture={capture}, testcase={testcase}"
            ),
        }
    }
}

impl Error for AdapterError {}

pub fn adapt_capture(
    capture: CapturedGame,
    catalog: &CardCatalog,
) -> Result<ReplayClassification, AdapterError> {
    let battle_id = capture.id;

    if capture.final_status == "playing" {
        return Ok(ReplayClassification::Skipped(SkippedReplay {
            battle_id,
            source_status: capture.final_status,
            reason: ReplaySkipReason::InProgress,
        }));
    }
    let Some(testcase) = capture.testcase.clone() else {
        return Ok(ReplayClassification::Skipped(SkippedReplay {
            battle_id,
            source_status: capture.final_status,
            reason: ReplaySkipReason::NoReplayTestcase,
        }));
    };

    let source_status = source_status(battle_id, &capture.final_status)?;
    let first_side = capture
        .first_player
        .ok_or_else(|| invalid(battle_id, "firstPlayer", "is null"))
        .and_then(|side| source_side(battle_id, "firstPlayer", side))?;
    let recording_side = capture
        .my_side
        .map(|side| source_side(battle_id, "mySide", side))
        .transpose()?;

    if capture.players.len() != 2 {
        return Err(invalid(
            battle_id,
            "players",
            format!("expected two players, found {}", capture.players.len()),
        ));
    }

    let mut players_by_side: [Option<CapturedPlayer>; 2] = std::array::from_fn(|_| None);
    for player in capture.players {
        let side = source_side(battle_id, "players[].side", player.side)?;
        let slot = &mut players_by_side[side.index()];
        if slot.replace(player).is_some() {
            return Err(invalid(
                battle_id,
                "players[].side",
                format!("side {} occurs more than once", side.index()),
            ));
        }
    }
    for (side, player) in players_by_side.iter().enumerate() {
        if player.is_none() {
            return Err(invalid(
                battle_id,
                "players[].side",
                format!("side {side} is missing"),
            ));
        }
    }

    let first_player = players_by_side[first_side.index()].take().unwrap();
    let second_player = players_by_side[first_side.other().index()].take().unwrap();
    let players = [
        normalize_player(
            battle_id,
            first_player,
            EnginePlayer::P1,
            first_side,
            catalog,
        )?,
        normalize_player(
            battle_id,
            second_player,
            EnginePlayer::P2,
            first_side.other(),
            catalog,
        )?,
    ];

    validate_testcase_header(
        battle_id,
        capture.night,
        capture.rounds.len(),
        &testcase,
        &players,
    )?;
    let rounds = normalize_rounds(battle_id, &capture.rounds, &testcase, &players, first_side)?;

    let room = capture.room.map(|room| SourceRoom {
        id: room.id,
        name: room.name,
        battle_rule_id: room.id_battle_rule,
        deck_format_id: room.id_deck_format,
    });
    let replay = ReplayCaseV1 {
        schema_version: REPLAY_SCHEMA_VERSION,
        metadata: ReplayMetadata {
            battle_id,
            captured_at: capture.captured_at,
            creation_time: capture.creation_time,
            room,
            battle_rule_id: capture.battle_rule_id,
            night: capture.night,
            source_status,
            source_first_side: first_side,
            recording_player_id: capture.my_id,
            recording_side,
            source_snapshot_count: capture.snapshots,
            source_issues: capture.issues,
        },
        players,
        rounds,
    };

    Ok(ReplayClassification::Ready(Box::new(replay)))
}

fn normalize_player(
    battle_id: u64,
    player: CapturedPlayer,
    engine_player: EnginePlayer,
    source_side: SourceSide,
    catalog: &CardCatalog,
) -> Result<ReplayPlayer, AdapterError> {
    if player.hand.len() != 4 {
        return Err(invalid(
            battle_id,
            format!("players[{}].hand", source_side.index()),
            format!("expected four cards, found {}", player.hand.len()),
        ));
    }

    let mut hand: [Option<ReplayCard>; 4] = std::array::from_fn(|_| None);
    for card in player.hand {
        let index = hand_index(
            battle_id,
            &format!("players[{}].hand[].index", source_side.index()),
            card.index,
        )?;
        if hand[index as usize].is_some() {
            return Err(invalid(
                battle_id,
                format!("players[{}].hand[].index", source_side.index()),
                format!("index {index} occurs more than once"),
            ));
        }

        let replay_card = normalize_card(battle_id, source_side, index, card, catalog)?;
        hand[index as usize] = Some(replay_card);
    }
    if let Some(index) = hand.iter().position(Option::is_none) {
        return Err(invalid(
            battle_id,
            format!("players[{}].hand[].index", source_side.index()),
            format!("index {index} is missing"),
        ));
    }
    let hand = hand.map(|card| card.unwrap());

    Ok(ReplayPlayer {
        engine_player,
        source_side,
        profile: SourceProfile {
            id: player.id,
            name: player.name,
            level: to_u16(battle_id, "players[].level", player.level)?,
            grade: player.grade,
            country: player.country,
            club: player.club.map(|club| SourceClub {
                id: club.id,
                name: club.name,
            }),
            registration_time: player.registration_time,
            certification_level: to_u16(
                battle_id,
                "players[].certificationLevel",
                player.certification_level,
            )?,
        },
        base_life: to_u16(battle_id, "players[].baseLife", player.base_life)?,
        base_pillz: to_u16(battle_id, "players[].basePillz", player.base_pillz)?,
        hand,
    })
}

fn normalize_card(
    battle_id: u64,
    source_side: SourceSide,
    hand_index: u8,
    card: CapturedCard,
    catalog: &CardCatalog,
) -> Result<ReplayCard, AdapterError> {
    let level = u8::try_from(card.level).map_err(|_| {
        invalid(
            battle_id,
            "players[].hand[].level",
            format!("{} is outside u8", card.level),
        )
    })?;
    let key = CardKey::new(card.id, level);
    if catalog.get(key).is_none() {
        return Err(AdapterError {
            battle_id,
            kind: AdapterErrorKind::UnknownCard {
                source_side,
                hand_index,
                key,
            },
        });
    }
    let source_name = card.name.ok_or_else(|| {
        invalid(
            battle_id,
            "players[].hand[].name",
            format!("is null for card {} level {}", key.id, key.level),
        )
    })?;
    let source_clan = card.clan.ok_or_else(|| {
        invalid(
            battle_id,
            "players[].hand[].clan",
            format!("is null for card {} level {}", key.id, key.level),
        )
    })?;

    Ok(ReplayCard {
        key,
        hand_index,
        source_name,
        source_clan,
        in_battle_id: card.in_battle_id,
        state: card.state,
        source_ability: card.ability.map(source_modifier),
        source_bonus: card.bonus.map(source_modifier),
    })
}

fn source_modifier(modifier: CapturedModifier) -> SourceModifier {
    SourceModifier {
        id: modifier.id,
        description: modifier.description,
    }
}

fn validate_testcase_header(
    battle_id: u64,
    capture_night: bool,
    capture_round_count: usize,
    testcase: &CapturedTestcase,
    players: &[ReplayPlayer; 2],
) -> Result<(), AdapterError> {
    if testcase.flip {
        return Err(invalid(
            battle_id,
            "testcase.flip",
            "must be false for a normalized capture",
        ));
    }
    compare(battle_id, "testcase.night", capture_night, testcase.night)?;
    if testcase.cards.len() != 8 || testcase.levels.len() != 8 {
        return Err(invalid(
            battle_id,
            "testcase.cards/levels",
            format!(
                "expected eight cards and levels, found {}/{}",
                testcase.cards.len(),
                testcase.levels.len()
            ),
        ));
    }

    compare(
        battle_id,
        "testcase.life/player1.baseLife",
        players[0].base_life as i64,
        testcase.life,
    )?;
    compare(
        battle_id,
        "testcase.pillz/player1.basePillz",
        players[0].base_pillz as i64,
        testcase.pillz,
    )?;

    for (player_index, player) in players.iter().enumerate() {
        for (hand_index, card) in player.hand.iter().enumerate() {
            let testcase_index = player_index * 4 + hand_index;
            compare(
                battle_id,
                &format!("testcase.cards[{testcase_index}]"),
                &card.source_name,
                &testcase.cards[testcase_index],
            )?;
            compare(
                battle_id,
                &format!("testcase.levels[{testcase_index}]"),
                card.key.level as i64,
                testcase.levels[testcase_index],
            )?;
        }
    }

    if testcase.moves.len() > 4 {
        return Err(invalid(
            battle_id,
            "testcase.moves",
            format!(
                "expected at most four rounds, found {}",
                testcase.moves.len()
            ),
        ));
    }
    if capture_round_count < testcase.moves.len() {
        return Err(invalid(
            battle_id,
            "rounds",
            format!(
                "capture has {} rounds but testcase has {} resolved moves",
                capture_round_count,
                testcase.moves.len()
            ),
        ));
    }

    Ok(())
}

fn normalize_rounds(
    battle_id: u64,
    rounds: &[super::capture::CapturedRound],
    testcase: &CapturedTestcase,
    players: &[ReplayPlayer; 2],
    first_side: SourceSide,
) -> Result<Vec<ReplayRound>, AdapterError> {
    let mut used_cards = [[false; 4]; 2];
    let mut normalized = Vec::with_capacity(testcase.moves.len());

    for (round_index, testcase_move) in testcase.moves.iter().enumerate() {
        let round = &rounds[round_index];
        compare(
            battle_id,
            &format!("rounds[{round_index}].round"),
            round.round,
            round_index as i64,
        )?;
        let actual_first_side = round
            .first
            .ok_or_else(|| invalid(battle_id, format!("rounds[{round_index}].first"), "is null"))
            .and_then(|side| {
                source_side(battle_id, &format!("rounds[{round_index}].first"), side)
            })?;
        if round_index == 0 {
            compare(
                battle_id,
                "rounds[0].first/firstPlayer",
                actual_first_side,
                first_side,
            )?;
        }
        if round.moves.len() != 2 {
            return Err(invalid(
                battle_id,
                format!("rounds[{round_index}].moves"),
                format!("expected two moves, found {}", round.moves.len()),
            ));
        }
        let move_sides = [
            source_side(
                battle_id,
                &format!("rounds[{round_index}].moves[0].side"),
                round.moves[0].side,
            )?,
            source_side(
                battle_id,
                &format!("rounds[{round_index}].moves[1].side"),
                round.moves[1].side,
            )?,
        ];
        let first_move_index = move_sides
            .iter()
            .position(|&side| side == actual_first_side)
            .ok_or_else(|| {
                invalid(
                    battle_id,
                    format!("rounds[{round_index}].moves[].side"),
                    format!("has no move for explicit first side {actual_first_side:?}"),
                )
            })?;
        let second_side = actual_first_side.other();
        let second_move_index = move_sides
            .iter()
            .position(|&side| side == second_side)
            .ok_or_else(|| {
                invalid(
                    battle_id,
                    format!("rounds[{round_index}].moves[].side"),
                    format!("has no move for opposite side {second_side:?}"),
                )
            })?;

        let first_play = normalize_play(
            battle_id,
            round_index,
            first_move_index,
            &round.moves[first_move_index],
            actual_first_side,
            first_side,
            players,
            &mut used_cards,
        )?;
        let second_play = normalize_play(
            battle_id,
            round_index,
            second_move_index,
            &round.moves[second_move_index],
            second_side,
            first_side,
            players,
            &mut used_cards,
        )?;
        let plays = [first_play, second_play];

        validate_testcase_selection(battle_id, round_index, "s1", &plays[0], testcase_move.s1)?;
        validate_testcase_selection(battle_id, round_index, "s2", &plays[1], testcase_move.s2)?;

        let expected_player_states = [
            expected_state(battle_id, round_index, EnginePlayer::P1, round, first_side)?,
            expected_state(battle_id, round_index, EnginePlayer::P2, round, first_side)?,
        ];
        compare_testcase_state(
            battle_id,
            round_index,
            &expected_player_states,
            testcase_move,
        )?;

        let expected_card_results = [
            expected_result(battle_id, round_index, EnginePlayer::P1, round, first_side)?,
            expected_result(battle_id, round_index, EnginePlayer::P2, round, first_side)?,
        ];
        compare_testcase_results(
            battle_id,
            round_index,
            &expected_card_results,
            testcase_move,
        )?;

        normalized.push(ReplayRound {
            round: round_index as u8,
            first_mover: engine_player(actual_first_side, first_side),
            source_first_side: actual_first_side,
            plays,
            expected_player_states,
            expected_card_results,
        });
    }

    Ok(normalized)
}

#[allow(clippy::too_many_arguments)]
fn normalize_play(
    battle_id: u64,
    round_index: usize,
    move_index: usize,
    source: &super::capture::CapturedMove,
    source_side: SourceSide,
    first_side: SourceSide,
    players: &[ReplayPlayer; 2],
    used_cards: &mut [[bool; 4]; 2],
) -> Result<ReplayPlay, AdapterError> {
    let engine_player = engine_player(source_side, first_side);
    let hand_index = hand_index(
        battle_id,
        &format!("rounds[{round_index}].moves[{move_index}].index"),
        source.index,
    )?;
    let hand_slot = &mut used_cards[engine_player.index()][hand_index as usize];
    if *hand_slot {
        return Err(invalid(
            battle_id,
            format!("rounds[{round_index}].moves[{move_index}].index"),
            format!("player {engine_player:?} already played hand index {hand_index}"),
        ));
    }
    *hand_slot = true;

    let card = &players[engine_player.index()].hand[hand_index as usize];
    compare(
        battle_id,
        &format!("rounds[{round_index}].moves[{move_index}].cardId"),
        source.card_id,
        card.key.id,
    )?;
    let pillz = to_u16(
        battle_id,
        &format!("rounds[{round_index}].moves[{move_index}].pillz"),
        source.pillz,
    )?;
    let source_pillz_used = to_u16(
        battle_id,
        &format!("rounds[{round_index}].moves[{move_index}].pillzUsed"),
        source.pillz_used,
    )?;
    let expected_source_pillz = pillz.checked_add(1).ok_or_else(|| {
        invalid(
            battle_id,
            format!("rounds[{round_index}].moves[{move_index}].pillz"),
            "cannot add the source's free pill within u16",
        )
    })?;
    if source_pillz_used != expected_source_pillz {
        return Err(invalid(
            battle_id,
            format!("rounds[{round_index}].moves[{move_index}].pillzUsed"),
            format!(
                "expected pillz + free pill = {expected_source_pillz}, found {source_pillz_used}"
            ),
        ));
    }

    Ok(ReplayPlay {
        engine_player,
        source_side,
        hand_index,
        card: card.key,
        pillz,
        source_pillz_used,
        fury: source.fury,
        source_timestamp_ms: source.t,
    })
}

fn validate_testcase_selection(
    battle_id: u64,
    round_index: usize,
    name: &str,
    play: &ReplayPlay,
    testcase: (i64, i64, bool),
) -> Result<(), AdapterError> {
    compare(
        battle_id,
        &format!("testcase.moves[{round_index}].{name}[0]"),
        play.hand_index as i64,
        testcase.0,
    )?;
    compare(
        battle_id,
        &format!("testcase.moves[{round_index}].{name}[1]"),
        play.pillz as i64,
        testcase.1,
    )?;
    compare(
        battle_id,
        &format!("testcase.moves[{round_index}].{name}[2]"),
        play.fury,
        testcase.2,
    )
}

fn expected_state(
    battle_id: u64,
    round_index: usize,
    player: EnginePlayer,
    round: &super::capture::CapturedRound,
    first_side: SourceSide,
) -> Result<ExpectedPlayerState, AdapterError> {
    let side = side_for_player(player, first_side);
    Ok(ExpectedPlayerState {
        life: to_u16(
            battle_id,
            &format!("rounds[{round_index}].life[{}]", side.index()),
            round.life[side.index()].ok_or_else(|| {
                invalid(
                    battle_id,
                    format!("rounds[{round_index}].life[{}]", side.index()),
                    "is null in the resolved prefix",
                )
            })?,
        )?,
        pillz: to_u16(
            battle_id,
            &format!("rounds[{round_index}].pillz[{}]", side.index()),
            round.pillz[side.index()].ok_or_else(|| {
                invalid(
                    battle_id,
                    format!("rounds[{round_index}].pillz[{}]", side.index()),
                    "is null in the resolved prefix",
                )
            })?,
        )?,
    })
}

fn compare_testcase_state(
    battle_id: u64,
    round_index: usize,
    states: &[ExpectedPlayerState; 2],
    testcase: &super::capture::CapturedTestcaseMove,
) -> Result<(), AdapterError> {
    let expected = [
        (testcase.p1life, testcase.p1pillz),
        (testcase.p2life, testcase.p2pillz),
    ];
    for player in 0..2 {
        compare(
            battle_id,
            &format!("testcase.moves[{round_index}].p{}life", player + 1),
            states[player].life as i64,
            expected[player].0,
        )?;
        compare(
            battle_id,
            &format!("testcase.moves[{round_index}].p{}pillz", player + 1),
            states[player].pillz as i64,
            expected[player].1,
        )?;
    }
    Ok(())
}

fn expected_result(
    battle_id: u64,
    round_index: usize,
    player: EnginePlayer,
    round: &super::capture::CapturedRound,
    first_side: SourceSide,
) -> Result<Option<ExpectedCardResult>, AdapterError> {
    let side = side_for_player(player, first_side);
    round.resolution[side.index()]
        .as_ref()
        .filter(|result| result.attack >= 0)
        .map(|result| normalize_result(battle_id, round_index, side, result))
        .transpose()
}

fn normalize_result(
    battle_id: u64,
    round_index: usize,
    source_side: SourceSide,
    result: &CapturedResolution,
) -> Result<ExpectedCardResult, AdapterError> {
    let field = format!("rounds[{round_index}].resolution[{}]", source_side.index());
    Ok(ExpectedCardResult {
        power: to_u16(battle_id, &format!("{field}.power"), result.power)?,
        damage: to_u16(battle_id, &format!("{field}.damage"), result.damage)?,
        attack: to_u32(battle_id, &format!("{field}.attack"), result.attack)?,
        won: result.won,
    })
}

fn compare_testcase_results(
    battle_id: u64,
    round_index: usize,
    results: &[Option<ExpectedCardResult>; 2],
    testcase: &super::capture::CapturedTestcaseMove,
) -> Result<(), AdapterError> {
    let testcase_results = [testcase.r1.as_ref(), testcase.r2.as_ref()];
    for player in 0..2 {
        let expected = testcase_results[player]
            .map(|result| normalize_testcase_result(battle_id, round_index, player, result))
            .transpose()?;
        compare(
            battle_id,
            &format!("testcase.moves[{round_index}].r{}", player + 1),
            results[player],
            expected,
        )?;
    }
    Ok(())
}

fn normalize_testcase_result(
    battle_id: u64,
    round_index: usize,
    player: usize,
    result: &CapturedTestcaseResult,
) -> Result<ExpectedCardResult, AdapterError> {
    let field = format!("testcase.moves[{round_index}].r{}", player + 1);
    Ok(ExpectedCardResult {
        power: to_u16(battle_id, &format!("{field}.power"), result.power)?,
        damage: to_u16(battle_id, &format!("{field}.damage"), result.damage)?,
        attack: to_u32(battle_id, &format!("{field}.attack"), result.attack)?,
        won: result.won,
    })
}

fn source_status(battle_id: u64, status: &str) -> Result<SourceStatus, AdapterError> {
    match status {
        "done" => Ok(SourceStatus::Done),
        "timeout" => Ok(SourceStatus::Timeout),
        "left" => Ok(SourceStatus::Left),
        _ => Err(invalid(
            battle_id,
            "finalStatus",
            format!("unsupported status {status:?}"),
        )),
    }
}

fn source_side(battle_id: u64, field: &str, value: i64) -> Result<SourceSide, AdapterError> {
    match value {
        0 => Ok(SourceSide::Side0),
        1 => Ok(SourceSide::Side1),
        _ => Err(invalid(
            battle_id,
            field,
            format!("expected side 0 or 1, found {value}"),
        )),
    }
}

fn engine_player(side: SourceSide, first_side: SourceSide) -> EnginePlayer {
    if side == first_side {
        EnginePlayer::P1
    } else {
        EnginePlayer::P2
    }
}

fn side_for_player(player: EnginePlayer, first_side: SourceSide) -> SourceSide {
    match player {
        EnginePlayer::P1 => first_side,
        EnginePlayer::P2 => first_side.other(),
    }
}

fn hand_index(battle_id: u64, field: &str, value: i64) -> Result<u8, AdapterError> {
    match value {
        0..=3 => Ok(value as u8),
        _ => Err(invalid(
            battle_id,
            field,
            format!("expected an index from 0 through 3, found {value}"),
        )),
    }
}

fn to_u16(battle_id: u64, field: &str, value: i64) -> Result<u16, AdapterError> {
    u16::try_from(value).map_err(|_| {
        invalid(
            battle_id,
            field,
            format!("{value} is outside the range 0..={}", u16::MAX),
        )
    })
}

fn to_u32(battle_id: u64, field: &str, value: i64) -> Result<u32, AdapterError> {
    u32::try_from(value).map_err(|_| {
        invalid(
            battle_id,
            field,
            format!("{value} is outside the range 0..={}", u32::MAX),
        )
    })
}

fn compare<T>(battle_id: u64, field: &str, capture: T, testcase: T) -> Result<(), AdapterError>
where
    T: fmt::Debug + PartialEq,
{
    if capture == testcase {
        Ok(())
    } else {
        Err(AdapterError {
            battle_id,
            kind: AdapterErrorKind::TestcaseMismatch {
                field: field.to_owned(),
                capture: format!("{capture:?}"),
                testcase: format!("{testcase:?}"),
            },
        })
    }
}

fn invalid(battle_id: u64, field: impl Into<String>, detail: impl Into<String>) -> AdapterError {
    AdapterError {
        battle_id,
        kind: AdapterErrorKind::InvalidField {
            field: field.into(),
            detail: detail.into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{adapt_capture, AdapterErrorKind, ReplayClassification, ReplaySkipReason};
    use crate::catalog::CardCatalog;
    use crate::replay::capture::CapturedGame;
    use crate::replay::model::{EnginePlayer, SourceSide};
    use std::fs::File;
    use std::path::PathBuf;

    fn root_path(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(path)
    }

    fn capture(id: u64) -> CapturedGame {
        CapturedGame::from_reader(
            File::open(root_path(&format!("captures/games/{id}.json"))).unwrap(),
        )
        .unwrap()
    }

    fn catalog() -> CardCatalog {
        CardCatalog::load(root_path("data/data.json")).unwrap()
    }

    #[test]
    fn normalizes_first_source_side_as_engine_player_one() {
        let classification = adapt_capture(capture(874590), &catalog()).unwrap();
        let ReplayClassification::Ready(replay) = classification else {
            panic!("expected ready replay");
        };

        assert_eq!(replay.metadata.source_first_side, SourceSide::Side1);
        assert_eq!(replay.players[0].engine_player, EnginePlayer::P1);
        assert_eq!(replay.players[0].source_side, SourceSide::Side1);
        assert_eq!(replay.players[1].source_side, SourceSide::Side0);
        assert_eq!(replay.players[0].profile.name, "DashSmashing");
        assert_eq!(replay.rounds[1].first_mover, EnginePlayer::P2);
        assert_eq!(replay.rounds[1].plays[0].engine_player, EnginePlayer::P2);
        assert_eq!(replay.rounds[1].plays[1].engine_player, EnginePlayer::P1);
    }

    #[test]
    fn orders_plays_from_explicit_first_side_when_source_array_is_reversed() {
        let mut source = capture(866431);
        source.rounds[0].moves.reverse();

        let classification = adapt_capture(source, &catalog()).unwrap();
        let ReplayClassification::Ready(replay) = classification else {
            panic!("expected ready replay");
        };

        assert_eq!(replay.rounds[0].plays[0].source_side, SourceSide::Side0);
        assert_eq!(replay.rounds[0].plays[0].hand_index, 3);
        assert_eq!(replay.rounds[0].plays[1].source_side, SourceSide::Side1);
        assert_eq!(replay.rounds[0].plays[1].hand_index, 1);
    }

    #[test]
    fn preserves_an_explicit_non_alternating_first_mover() {
        let mut source = capture(866431);
        source.rounds[1].first = Some(0);
        let testcase_move = &mut source.testcase.as_mut().unwrap().moves[1];
        let old_first = testcase_move.s1;
        testcase_move.s1 = testcase_move.s2;
        testcase_move.s2 = old_first;

        let classification = adapt_capture(source, &catalog()).unwrap();
        let ReplayClassification::Ready(replay) = classification else {
            panic!("expected ready replay");
        };

        assert_eq!(replay.rounds[1].first_mover, EnginePlayer::P1);
        assert_eq!(replay.rounds[1].plays[0].engine_player, EnginePlayer::P1);
        assert_eq!(replay.rounds[1].plays[1].engine_player, EnginePlayer::P2);
    }

    #[test]
    fn preserves_asymmetric_second_player_initial_resources() {
        let mut source = capture(866431);
        source.players[1].base_life = 14;
        source.players[1].base_pillz = 11;

        let classification = adapt_capture(source, &catalog()).unwrap();
        let ReplayClassification::Ready(replay) = classification else {
            panic!("expected ready replay");
        };

        assert_eq!(replay.players[0].base_life, 12);
        assert_eq!(replay.players[0].base_pillz, 12);
        assert_eq!(replay.players[1].base_life, 14);
        assert_eq!(replay.players[1].base_pillz, 11);
    }

    #[test]
    fn keeps_transient_resolution_damage_not_damage_after() {
        let classification = adapt_capture(capture(866431), &catalog()).unwrap();
        let ReplayClassification::Ready(replay) = classification else {
            panic!("expected ready replay");
        };

        let player_one = replay.rounds[0].expected_card_results[0].unwrap();
        assert_eq!(player_one.damage, 6);
        assert_ne!(player_one.damage, 3);
    }

    #[test]
    fn preserves_attack_beyond_u16_with_checked_u32_conversion() {
        let mut source = capture(866431);
        source.rounds[0].resolution[0].as_mut().unwrap().attack = 82_000;
        source.testcase.as_mut().unwrap().moves[0]
            .r1
            .as_mut()
            .unwrap()
            .attack = 82_000;

        let classification = adapt_capture(source, &catalog()).unwrap();
        let ReplayClassification::Ready(replay) = classification else {
            panic!("expected ready replay");
        };
        assert_eq!(
            replay.rounds[0].expected_card_results[0].unwrap().attack,
            82_000
        );
    }

    #[test]
    fn rejects_attack_beyond_u32() {
        let mut source = capture(866431);
        source.rounds[0].resolution[0].as_mut().unwrap().attack = i64::from(u32::MAX) + 1;

        let error = adapt_capture(source, &catalog()).unwrap_err();

        assert!(matches!(
            error.kind,
            AdapterErrorKind::InvalidField { field, .. }
                if field == "rounds[0].resolution[0].attack"
        ));
    }

    #[test]
    fn rejects_duplicate_source_player_sides() {
        let mut source = capture(866431);
        source.players[1].side = source.players[0].side;

        let error = adapt_capture(source, &catalog()).unwrap_err();

        assert!(matches!(error.kind, AdapterErrorKind::InvalidField { .. }));
    }

    #[test]
    fn rejects_move_card_that_disagrees_with_the_indexed_hand() {
        let mut source = capture(866431);
        source.rounds[0].moves[0].card_id = u32::MAX;

        let error = adapt_capture(source, &catalog()).unwrap_err();

        assert!(matches!(
            error.kind,
            AdapterErrorKind::TestcaseMismatch { field, .. }
                if field == "rounds[0].moves[0].cardId"
        ));
    }

    #[test]
    fn rejects_non_contiguous_resolved_rounds() {
        let mut source = capture(866431);
        source.rounds[1].round = 3;

        let error = adapt_capture(source, &catalog()).unwrap_err();

        assert!(matches!(
            error.kind,
            AdapterErrorKind::TestcaseMismatch { field, .. }
                if field == "rounds[1].round"
        ));
    }

    #[test]
    fn rejects_negative_resolved_resources() {
        let mut source = capture(866431);
        source.rounds[0].life[0] = Some(-1);

        let error = adapt_capture(source, &catalog()).unwrap_err();

        assert!(matches!(error.kind, AdapterErrorKind::InvalidField { .. }));
    }

    #[test]
    fn result_errors_name_the_actual_source_side_slot() {
        let mut source = capture(874590);
        source.rounds[0].resolution[1].as_mut().unwrap().power = -1;

        let error = adapt_capture(source, &catalog()).unwrap_err();

        assert!(matches!(
            error.kind,
            AdapterErrorKind::InvalidField { field, .. }
                if field == "rounds[0].resolution[1].power"
        ));
    }

    #[test]
    fn classifies_missing_testcases_and_in_progress_captures() {
        // Every committed capture carries a testcase now that Dojo battles are extracted
        // like any other, so the missing-testcase arm is exercised by clearing one rather
        // than by naming a battle that happens not to have been given one.
        let mut without = capture(830285);
        without.testcase = None;
        let no_testcase = adapt_capture(without, &catalog()).unwrap();
        let in_progress = adapt_capture(capture(957643), &catalog()).unwrap();

        assert!(matches!(
            no_testcase,
            ReplayClassification::Skipped(skip)
                if skip.reason == ReplaySkipReason::NoReplayTestcase
        ));
        assert!(matches!(
            in_progress,
            ReplayClassification::Skipped(skip)
                if skip.reason == ReplaySkipReason::InProgress
        ));
    }
}
