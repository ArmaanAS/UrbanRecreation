//! Terminal entrypoint for one-shot and manual-session Rust advice.
//!
//! The search and view layers are deliberately pure.  This binary is the only place that
//! reads process arguments, loads repository data, and writes a finished frame to stdout.

use std::env;
use std::error::Error;
use std::io::{self, BufRead, Write};
use std::process;
use std::time::Duration;

use urban_recreation_rust::advisor::input::{
    parse_args, prepare, AdvisorCommand, PreparedAdvisorInput, USAGE,
};
use urban_recreation_rust::advisor::search::{
    search, AdvisorMove, EvaluationKind, SearchConfig, SearchMode, SearchSnapshot,
};
use urban_recreation_rust::advisor::session::{AdvisorSession, ManualSelection};
use urban_recreation_rust::advisor::view::{
    render, AdvisorCard, AdvisorSide, AdvisorViewModel, ColourMode,
};
use urban_recreation_rust::engine::{
    BaseRulesRoundInput, BaseRulesRoundReport, BaseRulesSelection, ByPlayer, MatchStatus, PlayerId,
    HAND_SIZE,
};
use urban_recreation_rust::replay::{EnginePlayer, ReplayRound};

const MAX_PROMPT_LINE: usize = 64;

fn main() {
    if let Err(error) = run() {
        eprintln!("advisor: {}", safe_terminal_error(&error.to_string()));
        eprintln!("\n{USAGE}");
        process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    match parse_args(env::args().skip(1))? {
        AdvisorCommand::Help => {
            println!("{USAGE}");
            Ok(())
        }
        AdvisorCommand::Run(options) => {
            let prepared = prepare(options)?;
            if prepared.replay.is_some() {
                let stdout = io::stdout();
                run_replay(&prepared, &mut stdout.lock()).map_err(Into::into)
            } else if prepared.options.interactive {
                let stdin = io::stdin();
                let stdout = io::stdout();
                run_interactive(&prepared, &mut stdin.lock(), &mut stdout.lock())
                    .map_err(Into::into)
            } else {
                let stdout = io::stdout();
                run_once(&prepared, &mut stdout.lock()).map_err(Into::into)
            }
        }
    }
}

fn run_once(prepared: &PreparedAdvisorInput, output: &mut impl Write) -> io::Result<()> {
    let mut game = prepared.new_game();
    let config = search_config(prepared);
    render_search(prepared, &mut game, config, output).map(|_| ())
}

fn run_replay(prepared: &PreparedAdvisorInput, output: &mut impl Write) -> io::Result<()> {
    let replay = prepared
        .replay
        .as_ref()
        .expect("run_replay requires prepared replay data");
    let mut game = prepared.new_game();
    writeln!(
        output,
        "Replay {}: grading {} recorded decision(s) for {}.\n",
        replay.battle_id,
        replay.rounds.len(),
        safe_terminal_text(&replay.player_names[replay.us]),
    )?;

    for round in &replay.rounds {
        let expected_round = game.position().rounds_played;
        if round.round != expected_round {
            return Err(invalid_replay(format!(
                "battle {} expected normalized round {}, found {}",
                replay.battle_id, expected_round, round.round
            )));
        }
        if game.position().status != MatchStatus::Playing {
            return Err(invalid_replay(format!(
                "battle {} has another captured round after engine status {:?}",
                replay.battle_id,
                game.position().status
            )));
        }

        let first_mover = replay_player(round.first_mover);
        let mode = if first_mover == replay.us {
            SearchMode::First
        } else {
            SearchMode::Second {
                opponent_hand_index: captured_move(round, replay.us.other())?.hand_index,
            }
        };
        let config = SearchConfig {
            us: replay.us,
            first_mover,
            mode,
            budget: Duration::from_millis(prepared.options.budget_ms),
        };
        let snapshot = render_search(prepared, &mut game, config, output)?;
        let played = captured_move(round, replay.us)?;
        write_replay_grade(prepared, round, &snapshot, played, output)?;

        let input = captured_round_input(round)?;
        let (report, _undo) = game.make(input).map_err(|error| {
            invalid_replay(format!(
                "battle {} round {} failed in the strict engine: {error}",
                replay.battle_id,
                round.round + 1
            ))
        })?;
        verify_captured_round(replay.battle_id, round, &report)?;
        writeln!(
            output,
            "SERVER ROUND {} VERIFIED · P1 {}/{} · P2 {}/{} · {:?}\n",
            round.round + 1,
            report.players[PlayerId::P1].life,
            report.players[PlayerId::P1].pillz,
            report.players[PlayerId::P2].life,
            report.players[PlayerId::P2].pillz,
            report.status,
        )?;
    }

    let position = game.position();
    writeln!(
        output,
        "REPLAY {} COMPLETE · {} decisions graded · {:?} · P1 {}/{} · P2 {}/{}",
        replay.battle_id,
        replay.rounds.len(),
        position.status,
        position.players[PlayerId::P1].life,
        position.players[PlayerId::P1].pillz,
        position.players[PlayerId::P2].life,
        position.players[PlayerId::P2].pillz,
    )
}

fn captured_move(round: &ReplayRound, player: PlayerId) -> io::Result<AdvisorMove> {
    round
        .plays
        .iter()
        .find(|play| replay_player(play.engine_player) == player)
        .map(|play| AdvisorMove {
            hand_index: play.hand_index,
            pillz: play.pillz,
            fury: play.fury,
        })
        .ok_or_else(|| {
            invalid_replay(format!(
                "normalized round {} has no move for {player:?}",
                round.round + 1
            ))
        })
}

fn captured_round_input(round: &ReplayRound) -> io::Result<BaseRulesRoundInput> {
    let p1 = captured_move(round, PlayerId::P1)?;
    let p2 = captured_move(round, PlayerId::P2)?;
    Ok(BaseRulesRoundInput {
        first_mover: replay_player(round.first_mover),
        selections: ByPlayer::new(
            BaseRulesSelection::new(p1.hand_index, p1.pillz, p1.fury),
            BaseRulesSelection::new(p2.hand_index, p2.pillz, p2.fury),
        ),
    })
}

fn write_replay_grade(
    prepared: &PreparedAdvisorInput,
    round: &ReplayRound,
    snapshot: &SearchSnapshot,
    played: AdvisorMove,
    output: &mut impl Write,
) -> io::Result<()> {
    let us = prepared.options.us;
    let card = &prepared.cards[us][usize::from(played.hand_index)];
    let card_name = safe_terminal_text(&card.name);
    let wager = format!(
        "{} pillz{}",
        played.pillz,
        if played.fury { " + Fury" } else { "" }
    );
    let Some((index, row)) = snapshot
        .ranked
        .iter()
        .enumerate()
        .find(|(_, row)| row.move_ == played)
    else {
        return writeln!(
            output,
            "CAPTURED MOVE · round {} · {} · {} · not a legal solver action\n",
            round.round + 1,
            card_name,
            wager,
        );
    };
    if row.samples == 0 || !row.average.is_finite() {
        return writeln!(
            output,
            "CAPTURED MOVE · round {} · {} · {} · legal but not reached before budget\n",
            round.round + 1,
            card_name,
            wager,
        );
    }
    let best = snapshot
        .ranked
        .iter()
        .find(|candidate| candidate.samples > 0 && candidate.average.is_finite())
        .map(|candidate| displayed_percent(candidate.average))
        .unwrap_or_else(|| "--".to_owned());
    writeln!(
        output,
        "CAPTURED MOVE · round {} · {} · {} · rank {}/{} · score {} · best {}{}\n",
        round.round + 1,
        card_name,
        wager,
        index + 1,
        snapshot.ranked.len(),
        displayed_percent(row.average),
        best,
        if snapshot.complete { "" } else { " · partial" },
    )
}

fn verify_captured_round(
    battle_id: u64,
    expected: &ReplayRound,
    actual: &BaseRulesRoundReport,
) -> io::Result<()> {
    for player in PlayerId::ALL {
        let index = match player {
            PlayerId::P1 => 0,
            PlayerId::P2 => 1,
        };
        let expected_player = expected.expected_player_states[index];
        let actual_player = actual.players[player];
        if actual_player.life != expected_player.life
            || actual_player.pillz != expected_player.pillz
        {
            return Err(invalid_replay(format!(
                "battle {battle_id} round {} {player:?} resource mismatch: engine {}/{} server {}/{}",
                expected.round + 1,
                actual_player.life,
                actual_player.pillz,
                expected_player.life,
                expected_player.pillz,
            )));
        }
        if let Some(expected_card) = expected.expected_card_results[index] {
            let actual_card = actual.cards[player];
            if actual_card.power != expected_card.power
                || actual_card.damage != expected_card.damage
                || actual_card.attack != expected_card.attack
                || actual_card.won != expected_card.won
            {
                return Err(invalid_replay(format!(
                    "battle {battle_id} round {} {player:?} card mismatch: engine {}/{}/{} won={} server {}/{}/{} won={}",
                    expected.round + 1,
                    actual_card.power,
                    actual_card.damage,
                    actual_card.attack,
                    actual_card.won,
                    expected_card.power,
                    expected_card.damage,
                    expected_card.attack,
                    expected_card.won,
                )));
            }
        }
    }
    Ok(())
}

fn replay_player(player: EnginePlayer) -> PlayerId {
    match player {
        EnginePlayer::P1 => PlayerId::P1,
        EnginePlayer::P2 => PlayerId::P2,
    }
}

fn displayed_percent(value: f64) -> String {
    if !value.is_finite() {
        return "--".to_owned();
    }
    let percent = ((value.clamp(-1.0, 1.0) + 1.0) * 50.0).round() as i32;
    let percent = if percent >= 100 && value < 1.0 {
        99
    } else if percent <= 0 && value > -1.0 {
        1
    } else {
        percent
    };
    format!("{percent}%")
}

fn invalid_replay(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn safe_terminal_text(value: &str) -> String {
    safe_terminal_text_bounded(value, 64)
}

fn safe_terminal_error(value: &str) -> String {
    safe_terminal_text_bounded(value, 1024)
}

fn safe_terminal_text_bounded(value: &str, limit: usize) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                '?'
            } else {
                character
            }
        })
        .take(limit)
        .collect()
}

fn run_interactive(
    prepared: &PreparedAdvisorInput,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> io::Result<()> {
    let mut session = AdvisorSession::new(prepared.new_game(), prepared.options.first_mover);
    writeln!(
        output,
        "Manual session: enter SLOT:PILLZ or SLOT:PILLZ:F; enter q to stop."
    )?;

    while session.status() == MatchStatus::Playing {
        let first_mover = session.current_first_mover();
        let revealed = if first_mover == prepared.options.us {
            None
        } else {
            match prompt_revealed_card(prepared.options.us, &session, input, output)? {
                Some(slot) => Some(slot),
                None => {
                    writeln!(
                        output,
                        "Session stopped without changing the current round."
                    )?;
                    return Ok(());
                }
            }
        };
        let mode = revealed.map_or(SearchMode::First, |opponent_hand_index| {
            SearchMode::Second {
                opponent_hand_index,
            }
        });
        let config = SearchConfig {
            us: prepared.options.us,
            first_mover,
            mode,
            budget: Duration::from_millis(prepared.options.budget_ms),
        };
        render_search(prepared, session.game_mut(), config, output)?;

        loop {
            let Some(ours) = prompt_selection("Your resolved move", input, output)? else {
                writeln!(
                    output,
                    "Session stopped without changing the current round."
                )?;
                return Ok(());
            };
            let Some(theirs) = prompt_selection("Opponent resolved move", input, output)? else {
                writeln!(
                    output,
                    "Session stopped without changing the current round."
                )?;
                return Ok(());
            };
            if let Some(slot) = revealed {
                if theirs.hand_index != slot {
                    writeln!(
                        output,
                        "That opponent move uses slot {}, but slot {slot} was revealed; enter the round again.",
                        theirs.hand_index
                    )?;
                    continue;
                }
            }
            let selections = match prepared.options.us {
                PlayerId::P1 => ByPlayer::new(ours, theirs),
                PlayerId::P2 => ByPlayer::new(theirs, ours),
            };
            match session.commit_round(selections) {
                Ok(report) => {
                    writeln!(
                        output,
                        "ROUND {} RESOLVED · P1 attack {} · P2 attack {} · P1 {}/{} · P2 {}/{} · {:?}\n",
                        report.round + 1,
                        report.cards[PlayerId::P1].attack,
                        report.cards[PlayerId::P2].attack,
                        report.players[PlayerId::P1].life,
                        report.players[PlayerId::P1].pillz,
                        report.players[PlayerId::P2].life,
                        report.players[PlayerId::P2].pillz,
                        report.status,
                    )?;
                    break;
                }
                Err(error) => {
                    writeln!(output, "{error}; enter both observed moves again.")?;
                }
            }
        }
    }

    let position = session.game().position();
    writeln!(
        output,
        "MATCH COMPLETE · {:?} · P1 {}/{} · P2 {}/{}",
        position.status,
        position.players[PlayerId::P1].life,
        position.players[PlayerId::P1].pillz,
        position.players[PlayerId::P2].life,
        position.players[PlayerId::P2].pillz,
    )
}

fn render_search(
    prepared: &PreparedAdvisorInput,
    game: &mut urban_recreation_rust::engine::CombatStatDiagnosticV1,
    config: SearchConfig,
    output: &mut impl Write,
) -> io::Result<SearchSnapshot> {
    let mode = config.mode;
    let snapshot = search(game, config, |_| {});
    let model = view_model(prepared, game, snapshot.clone(), mode);
    let colour = if prepared.options.plain {
        ColourMode::Never
    } else {
        ColourMode::Always
    };
    writeln!(
        output,
        "{}",
        render(
            &model,
            usize::from(prepared.options.width),
            usize::from(prepared.options.height),
            colour,
        )
    )?;
    Ok(snapshot)
}

fn prompt_revealed_card(
    us: PlayerId,
    session: &AdvisorSession,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> io::Result<Option<u8>> {
    loop {
        let Some(line) = prompt_line("Opponent revealed card slot (0..3)", input, output)? else {
            return Ok(None);
        };
        let Ok(slot) = line.parse::<u8>() else {
            writeln!(output, "Card slot must be an integer in 0..3.")?;
            continue;
        };
        if slot >= HAND_SIZE as u8 {
            writeln!(output, "Card slot must be an integer in 0..3.")?;
            continue;
        }
        if session.game().position().played[us.other()][usize::from(slot)] {
            writeln!(output, "Opponent slot {slot} was already played.")?;
            continue;
        }
        return Ok(Some(slot));
    }
}

fn prompt_selection(
    label: &str,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> io::Result<Option<ManualSelection>> {
    loop {
        let Some(line) = prompt_line(label, input, output)? else {
            return Ok(None);
        };
        match ManualSelection::parse(&line) {
            Ok(selection) => return Ok(Some(selection)),
            Err(error) => writeln!(output, "{error}")?,
        }
    }
}

fn prompt_line(
    label: &str,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> io::Result<Option<String>> {
    loop {
        write!(output, "{label} [q to stop]: ")?;
        output.flush()?;
        match read_bounded_line(input)? {
            PromptLine::Eof => return Ok(None),
            PromptLine::TooLong => {
                writeln!(output, "Input must be at most {MAX_PROMPT_LINE} bytes.")?;
            }
            PromptLine::Value(line) if line.eq_ignore_ascii_case("q") => return Ok(None),
            PromptLine::Value(line) => return Ok(Some(line)),
        }
    }
}

enum PromptLine {
    Eof,
    TooLong,
    Value(String),
}

fn read_bounded_line(input: &mut impl BufRead) -> io::Result<PromptLine> {
    let mut bytes = Vec::with_capacity(MAX_PROMPT_LINE);
    let mut too_long = false;
    let mut saw_any = false;
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            if !saw_any {
                return Ok(PromptLine::Eof);
            }
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |index| index + 1);
        for &byte in &available[..take] {
            saw_any = true;
            if byte == b'\n' {
                break;
            }
            if bytes.len() < MAX_PROMPT_LINE {
                bytes.push(byte);
            } else {
                too_long = true;
            }
        }
        input.consume(take);
        if newline.is_some() {
            break;
        }
    }
    if too_long {
        return Ok(PromptLine::TooLong);
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    Ok(PromptLine::Value(
        String::from_utf8_lossy(&bytes).trim().to_owned(),
    ))
}

fn search_config(prepared: &PreparedAdvisorInput) -> SearchConfig {
    let options = &prepared.options;
    let mode = match options.second_card {
        Some(opponent_hand_index) => SearchMode::Second {
            opponent_hand_index,
        },
        None => SearchMode::First,
    };
    SearchConfig {
        us: options.us,
        first_mover: options.first_mover,
        mode,
        budget: Duration::from_millis(options.budget_ms),
    }
}

fn view_model(
    prepared: &PreparedAdvisorInput,
    game: &urban_recreation_rust::engine::CombatStatDiagnosticV1,
    snapshot: urban_recreation_rust::advisor::search::SearchSnapshot,
    mode: SearchMode,
) -> AdvisorViewModel {
    let phase = match snapshot.evaluation {
        EvaluationKind::OneRoundHeuristic => "ONE-ROUND HEURISTIC",
        EvaluationKind::ExactLatePolicy => "EXACT LATE POLICY",
    };
    let limitation = match snapshot.evaluation {
        EvaluationKind::OneRoundHeuristic => {
            if prepared.replay.is_some() {
                "server-backed strict replay; future rounds use a position heuristic; current choices uniform"
            } else {
                "strict supported draw; future rounds use a position heuristic; current choices uniform"
            }
        }
        EvaluationKind::ExactLatePolicy => {
            if prepared.replay.is_some() {
                "server-backed strict replay; exact rounds 3-4 policy; current hidden choices uniform"
            } else {
                "strict supported draw; exact rounds 3-4 policy; current hidden choices uniform"
            }
        }
    };
    let round = game.position().rounds_played + 1;
    let replay_prefix = prepared
        .replay
        .as_ref()
        .map(|replay| format!("REPLAY {} · ROUND {round} · ", replay.battle_id))
        .unwrap_or_default();
    AdvisorViewModel {
        p1: view_side(prepared, game, PlayerId::P1),
        p2: view_side(prepared, game, PlayerId::P2),
        us: prepared.options.us,
        mode: match mode {
            SearchMode::Second {
                opponent_hand_index,
            } => format!("{replay_prefix}{phase} · SECOND · OPP CARD {opponent_hand_index}"),
            SearchMode::First => format!("{replay_prefix}{phase} · FIRST"),
        },
        limitation: limitation.to_owned(),
        snapshot,
        supported_cards: HAND_SIZE * PlayerId::ALL.len(),
        provenance_revision: prepared
            .combat_match
            .provenance()
            .compiler_policy_semantic_revision,
    }
}

fn view_side(
    prepared: &PreparedAdvisorInput,
    game: &urban_recreation_rust::engine::CombatStatDiagnosticV1,
    player: PlayerId,
) -> AdvisorSide {
    let position = game.position();
    let display = &prepared.cards[player];
    let cards = std::array::from_fn(|slot| {
        let card = &display[slot];
        AdvisorCard {
            name: card.name.clone(),
            id: card.key.id,
            level: card.key.level,
            power: u16::from(card.power),
            damage: u16::from(card.damage),
            played: position.played[player][slot],
        }
    });
    AdvisorSide {
        label: if let Some(replay) = &prepared.replay {
            format!(
                "{} · {}",
                prepared.source_label, replay.player_names[player]
            )
        } else {
            prepared.source_label.clone()
        },
        life: position.players[player].life,
        pillz: position.players[player].pillz,
        cards,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        read_bounded_line, run_interactive, run_replay, safe_terminal_error, search_config,
        AdvisorCommand, PromptLine,
    };
    use std::io::Cursor;
    use urban_recreation_rust::advisor::input::{parse_args, prepare, AdvisorOptions};
    use urban_recreation_rust::advisor::search::SearchMode;
    use urban_recreation_rust::engine::PlayerId;

    #[test]
    fn demo_preparation_uses_the_strict_catalog_boundary() {
        let prepared = prepare(AdvisorOptions::default()).expect("demo draw must stay supported");
        assert_eq!(prepared.combat_match.input().battle_rule_id, 10);
        assert_eq!(prepared.combat_match.preparation().0.len(), 2);
        assert_eq!(prepared.cards[PlayerId::P1][0].name, "Natrang");
    }

    #[test]
    fn revealed_second_card_selects_the_second_mover_search_mode() {
        let AdvisorCommand::Run(options) = parse_args(
            ["--us", "p2", "--first", "p1", "--second-card", "2"]
                .into_iter()
                .map(str::to_owned),
        )
        .unwrap() else {
            panic!("the invocation must run");
        };
        let prepared = prepare(options).unwrap();
        assert_eq!(
            search_config(&prepared).mode,
            SearchMode::Second {
                opponent_hand_index: 2
            }
        );
    }

    #[test]
    fn bounded_reader_discards_an_overlong_line_without_losing_the_next_one() {
        let mut bytes = vec![b'x'; 65];
        bytes.extend_from_slice(b"\n0:0\n");
        let mut input = Cursor::new(bytes);
        assert!(matches!(
            read_bounded_line(&mut input).unwrap(),
            PromptLine::TooLong
        ));
        match read_bounded_line(&mut input).unwrap() {
            PromptLine::Value(line) => assert_eq!(line, "0:0"),
            _ => panic!("the bounded reader must resume at the next line"),
        }
    }

    #[test]
    fn manual_demo_advances_all_four_rounds_through_real_search_and_engine() {
        let mut options = AdvisorOptions::default();
        options.interactive = true;
        options.plain = true;
        options.budget_ms = 1;
        let prepared = prepare(options).unwrap();
        // P1 starts rounds 1/3. P2 starts rounds 2/4, so their visible card is supplied
        // before those searches. Both resolved moves are then recorded in our/their order.
        let script = b"0:0\n0:0\n1\n1:0\n1:0\n2:0\n2:0\n3\n3:0\n3:0\n";
        let mut input = Cursor::new(script);
        let mut output = Vec::new();
        run_interactive(&prepared, &mut input, &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("ONE-ROUND HEURISTIC · FIRST"));
        assert!(output.contains("ONE-ROUND HEURISTIC · SECOND · OPP CARD 1"));
        assert!(output.contains("EXACT LATE POLICY · FIRST"));
        assert!(output.contains("ROUND 4 RESOLVED"));
        assert!(output.contains("MATCH COMPLETE · Won(P2)"));
    }

    #[test]
    fn captured_battle_runs_first_and_second_advice_and_verifies_every_server_round() {
        let AdvisorCommand::Run(options) = parse_args(
            [
                "--replay",
                "877636",
                "--plain",
                "--budget-ms",
                "1",
                "--width",
                "100",
                "--height",
                "24",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .unwrap() else {
            panic!("the replay invocation must run");
        };
        let mut prepared = prepare(options).expect("877636 is the first strict real replay draw");
        assert_eq!(prepared.options.us, PlayerId::P2);
        assert_eq!(prepared.replay.as_ref().unwrap().rounds.len(), 4);
        for card in &mut prepared.cards[prepared.options.us] {
            card.name = "safe\nFORGED\u{1b}[31m\rname".to_owned();
        }

        let mut output = Vec::new();
        run_replay(&prepared, &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("ROUND 1 · ONE-ROUND HEURISTIC · SECOND · OPP CARD 2"));
        assert!(output.contains("ROUND 2 · ONE-ROUND HEURISTIC · FIRST"));
        assert!(output.contains("ROUND 3 · EXACT LATE POLICY · SECOND · OPP CARD 1"));
        assert!(output.contains("SERVER ROUND 4 VERIFIED"));
        assert!(output.contains("REPLAY 877636 COMPLETE · 4 decisions graded · Won(P1)"));
        assert!(!output.contains('\u{1b}'));
        assert!(!output.contains("\nFORGED"));
    }

    #[test]
    fn terminal_error_text_is_single_line_and_bounded() {
        let unsafe_text = format!("bad\u{1b}[31m\r\n{}", "x".repeat(2_000));
        let safe = safe_terminal_error(&unsafe_text);
        assert!(!safe.contains('\u{1b}'));
        assert!(!safe.contains('\r'));
        assert!(!safe.contains('\n'));
        assert_eq!(safe.chars().count(), 1_024);
    }
}
