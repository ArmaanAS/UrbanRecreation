//! Wall-clock cost of the root search at one thread and at every hardware thread, on the
//! decisions `docs/rust-migration.md` times. Ignored because it takes minutes; run with
//!
//!   cargo test --manifest-path rust/Cargo.toml --release --locked \
//!     --test advisor_parallel_timing -- --ignored --nocapture
//!
//! Samples alternate between the two counts and the table reports medians, because the
//! machine may be shared. Every sample is also checked bit-for-bit against the one-thread
//! result: a faster answer that differs is not a faster answer.

use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use urban_recreation_rust::advisor::input::{prepare, AdvisorOptions, PreparedAdvisorInput};
use urban_recreation_rust::advisor::search::{
    default_search_threads, search_with_threads, AdvisorMove, OpeningPolicy, RankedMove,
    SearchConfig, SearchMode, SearchSnapshot,
};
use urban_recreation_rust::engine::{
    BaseRulesRoundInput, BaseRulesSelection, ByPlayer, CombatStatDiagnosticV1, PlayerId,
};
use urban_recreation_rust::replay::model::{EnginePlayer, ReplayRound};

const SAMPLES: usize = 3;

fn engine(player: EnginePlayer) -> PlayerId {
    match player {
        EnginePlayer::P1 => PlayerId::P1,
        EnginePlayer::P2 => PlayerId::P2,
    }
}

fn captured_input(round: &ReplayRound) -> BaseRulesRoundInput {
    let selection = |player: EnginePlayer| {
        let play = round
            .plays
            .iter()
            .find(|play| play.engine_player == player)
            .unwrap();
        BaseRulesSelection::new(play.hand_index, play.pillz, play.fury)
    };
    BaseRulesRoundInput {
        first_mover: engine(round.first_mover),
        selections: ByPlayer::new(selection(EnginePlayer::P1), selection(EnginePlayer::P2)),
    }
}

fn prepared(replay: Option<u64>) -> PreparedAdvisorInput {
    prepare(AdvisorOptions {
        replay_id: replay,
        ..AdvisorOptions::default()
    })
    .unwrap()
}

struct Decision {
    label: String,
    game: CombatStatDiagnosticV1,
    config: SearchConfig,
}

fn config(us: PlayerId, first_mover: PlayerId, mode: SearchMode) -> SearchConfig {
    SearchConfig {
        us,
        first_mover,
        mode,
        budget: Duration::from_secs(600),
        opening: OpeningPolicy::ExactContinuation,
    }
}

/// FIRST is the round's first mover; SECOND is the other side, shown the card the first
/// mover actually played.
fn both_information_sets(
    label: &str,
    game: CombatStatDiagnosticV1,
    round: &ReplayRound,
) -> [Decision; 2] {
    let first = engine(round.first_mover);
    let revealed = round.plays[0].hand_index;
    [
        Decision {
            label: format!("{label} FIRST"),
            game: game.clone(),
            config: config(first, first, SearchMode::First),
        },
        Decision {
            label: format!("{label} SECOND"),
            game,
            config: config(
                first.other(),
                first,
                SearchMode::Second {
                    opponent_hand_index: revealed,
                },
            ),
        },
    ]
}

fn bits(
    snapshot: &SearchSnapshot,
) -> Vec<(AdvisorMove, [u64; 3], [usize; 3], Vec<(u16, bool, u64, u8)>)> {
    snapshot
        .ranked
        .iter()
        .map(|row: &RankedMove| {
            (
                row.move_,
                [
                    row.average.to_bits(),
                    row.worst.to_bits(),
                    row.best.to_bits(),
                ],
                [row.samples, row.kos, row.koed],
                row.hidden_outcomes
                    .iter()
                    .map(|outcome| {
                        (
                            outcome.opponent_pillz,
                            outcome.opponent_fury,
                            outcome.value.to_bits(),
                            outcome.flags,
                        )
                    })
                    .collect(),
            )
        })
        .collect()
}

fn median(mut values: Vec<Duration>) -> Duration {
    values.sort();
    values[values.len() / 2]
}

#[test]
#[ignore = "minutes of release-mode search; run by hand for the migration doc's tables"]
fn exact_opening_and_round_two_timings_at_one_thread_and_every_thread() {
    let mut decisions = Vec::new();
    let demo = prepared(None);
    let demo_game = demo.new_game();
    decisions.push(Decision {
        label: "demo opening FIRST".to_owned(),
        game: demo_game.clone(),
        config: config(PlayerId::P1, PlayerId::P1, SearchMode::First),
    });
    for id in [925719, 1024673, 1089346] {
        let capture = prepared(Some(id));
        let round = &capture.replay.as_ref().unwrap().rounds[0];
        decisions.extend(both_information_sets(
            &format!("{id} opening"),
            capture.new_game(),
            round,
        ));
    }
    let capture = prepared(Some(877636));
    let rounds = &capture.replay.as_ref().unwrap().rounds;
    let mut game = capture.new_game();
    game.make(captured_input(&rounds[0])).unwrap();
    let first = engine(rounds[1].first_mover);
    decisions.push(Decision {
        label: "877636 round 2 exact FIRST".to_owned(),
        game,
        config: config(first, first, SearchMode::First),
    });

    let many = default_search_threads();
    println!("threads: 1 vs {many}; {SAMPLES} alternating samples, medians");
    println!("| Decision | Units | 1 thread | {many} threads | Speedup |");
    println!("| --- | --- | --- | --- | --- |");
    for mut decision in decisions {
        let before = decision.game.clone();
        let mut reference = None;
        let mut times = [Vec::new(), Vec::new()];
        let mut units = 0;
        for _ in 0..SAMPLES {
            for (slot, threads) in [NonZeroUsize::MIN, many].into_iter().enumerate() {
                let started = Instant::now();
                let result =
                    search_with_threads(&mut decision.game, decision.config, threads, |_| {});
                times[slot].push(started.elapsed());
                assert!(result.complete, "{}: incomplete", decision.label);
                assert_eq!(decision.game, before, "{}: root moved", decision.label);
                units = result.units_total;
                let bits = bits(&result);
                match &reference {
                    None => reference = Some(bits),
                    Some(reference) => {
                        assert_eq!(&bits, reference, "{}: result differs", decision.label)
                    }
                }
            }
        }
        let [one, all] = times.map(median);
        let shown = |time: Duration| {
            if time < Duration::from_secs(1) {
                format!("{} ms", time.as_millis())
            } else {
                format!("{:.1} s", time.as_secs_f64())
            }
        };
        println!(
            "| {} | {units} | {} | {} | {:.1}x |",
            decision.label,
            shown(one),
            shown(all),
            one.as_secs_f64() / all.as_secs_f64(),
        );
    }
}
