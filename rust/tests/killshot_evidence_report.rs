//! What the corpus actually knows about Killshot.
//!
//! Killshot fires when the owner's final attack is at least twice the opponent's. Fifteen
//! Killshot definitions block draws, and none of them can be written honestly until the
//! server has been seen paying one out: a round where the ability was selected but the
//! attack ratio fell short pins only the negative half, that an ordinary win pays nothing.
//!
//! This report separates those two populations over every replayable capture, so that a
//! newly captured battle can be checked with one command rather than by hand:
//!
//! ```text
//! cargo test --manifest-path rust/Cargo.toml --locked \
//!     --test killshot_evidence_report -- --ignored --nocapture
//! ```
//!
//! A round counts as *firing* only when the capture reports both attacks and the owner's is
//! at least twice the opponent's. Captures without card results cannot answer the question
//! either way and are counted separately rather than assumed.
use std::collections::BTreeMap;
use std::path::PathBuf;

use urban_recreation_rust::replay::{load_corpus, EnginePlayer, ReplayCaseV1};

fn root_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(path)
}

fn player_index(player: EnginePlayer) -> usize {
    match player {
        EnginePlayer::P1 => 0,
        EnginePlayer::P2 => 1,
    }
}

struct KillshotRound {
    battle: u64,
    round: usize,
    side: usize,
    card: String,
    source: String,
    definition: u32,
    attack: Option<u32>,
    opponent_attack: Option<u32>,
    won: Option<bool>,
    life_before: [u16; 2],
    life_after: [u16; 2],
    pillz_before: [u16; 2],
    pillz_after: [u16; 2],
}

impl KillshotRound {
    /// `Some(true)` when the capture reports both attacks and the owner's is at least
    /// twice the opponent's; `None` when the capture cannot answer.
    fn fired(&self) -> Option<bool> {
        match (self.attack, self.opponent_attack) {
            (Some(ours), Some(theirs)) => Some(ours >= theirs.saturating_mul(2)),
            _ => None,
        }
    }
}

fn collect(case: &ReplayCaseV1) -> Vec<KillshotRound> {
    let mut found = Vec::new();
    for (index, round) in case.rounds.iter().enumerate() {
        let before = if index == 0 {
            [case.players[0].base_life, case.players[1].base_life]
        } else {
            let previous = &case.rounds[index - 1];
            [
                previous.expected_player_states[0].life,
                previous.expected_player_states[1].life,
            ]
        };
        let pillz_before = if index == 0 {
            [case.players[0].base_pillz, case.players[1].base_pillz]
        } else {
            let previous = &case.rounds[index - 1];
            [
                previous.expected_player_states[0].pillz,
                previous.expected_player_states[1].pillz,
            ]
        };
        for play in &round.plays {
            let side = player_index(play.engine_player);
            let card = &case.players[side].hand[usize::from(play.hand_index)];
            for source in [&card.source_ability, &card.source_bonus] {
                let Some(source) = source else { continue };
                if !source.description.contains("Killshot") {
                    continue;
                }
                found.push(KillshotRound {
                    battle: case.metadata.battle_id,
                    round: index,
                    side,
                    card: card.source_name.clone(),
                    source: source.description.clone(),
                    definition: source.id,
                    attack: round.expected_card_results[side].map(|result| result.attack),
                    opponent_attack: round.expected_card_results[1 - side]
                        .map(|result| result.attack),
                    won: round.expected_card_results[side].map(|result| result.won),
                    life_before: before,
                    life_after: [
                        round.expected_player_states[0].life,
                        round.expected_player_states[1].life,
                    ],
                    pillz_before,
                    pillz_after: [
                        round.expected_player_states[0].pillz,
                        round.expected_player_states[1].pillz,
                    ],
                });
            }
        }
    }
    found
}

#[test]
#[ignore = "report, not a gate"]
fn killshot_rounds_are_separated_into_firing_and_non_firing_evidence() {
    let corpus = load_corpus(root_path("captures/games"), root_path("data/data.json")).unwrap();
    let mut rounds: Vec<KillshotRound> = corpus.ready.iter().flat_map(collect).collect();
    rounds.sort_by_key(|round| (round.battle, round.round, round.side));

    let mut by_definition: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
    let (mut fired, mut missed, mut unknown) = (0, 0, 0);
    for round in &rounds {
        let entry = by_definition.entry(round.definition).or_insert((0, 0));
        entry.0 += 1;
        match round.fired() {
            Some(true) => {
                fired += 1;
                entry.1 += 1;
            }
            Some(false) => missed += 1,
            None => unknown += 1,
        }
    }

    println!("Killshot sources selected in {} rounds", rounds.len());
    println!("  attack at least doubled (pays):   {fired}");
    println!("  attack fell short (pins nothing): {missed}");
    println!("  capture cannot say:               {unknown}");

    println!("\n-- rounds where Killshot actually fired --");
    if fired == 0 {
        println!("  (none - nothing in the corpus can pin a Killshot magnitude)");
    }
    for round in rounds.iter().filter(|round| round.fired() == Some(true)) {
        println!(
            "  {}/{} side {} {} [{} {}]",
            round.battle, round.round, round.side, round.card, round.definition, round.source
        );
        println!(
            "      attack {:?} vs {:?}, won {:?}",
            round.attack, round.opponent_attack, round.won
        );
        println!(
            "      life {:?} -> {:?}, pillz {:?} -> {:?}",
            round.life_before, round.life_after, round.pillz_before, round.pillz_after
        );
    }

    println!("\n-- per definition: selected / of which fired --");
    for (definition, (selected, paid)) in &by_definition {
        println!("  {definition:5}  {selected:3} / {paid}");
    }

    println!("\n-- non-firing rounds, closest first --");
    let mut near: Vec<&KillshotRound> = rounds
        .iter()
        .filter(|round| round.fired() == Some(false))
        .collect();
    near.sort_by_key(|round| {
        let ours = round.attack.unwrap_or(0) as i64;
        let theirs = round.opponent_attack.unwrap_or(0) as i64;
        theirs * 2 - ours
    });
    for round in near.iter().take(10) {
        println!(
            "  {}/{} {} attack {:?} needed {:?}",
            round.battle,
            round.round,
            round.card,
            round.attack,
            round.opponent_attack.map(|attack| attack * 2)
        );
    }
}
