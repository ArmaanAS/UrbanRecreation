//! One-shot terminal entrypoint for the current Rust advisor.
//!
//! The search and view layers are deliberately pure.  This binary is the only place that
//! reads process arguments, loads repository data, and writes a finished frame to stdout.

use std::env;
use std::error::Error;
use std::process;
use std::time::Duration;

use urban_recreation_rust::advisor::input::{
    parse_args, prepare, AdvisorCommand, PreparedAdvisorInput, USAGE,
};
use urban_recreation_rust::advisor::search::{search, SearchConfig, SearchMode};
use urban_recreation_rust::advisor::view::{
    render, AdvisorCard, AdvisorSide, AdvisorViewModel, ColourMode,
};
use urban_recreation_rust::engine::{PlayerId, HAND_SIZE};

fn main() {
    if let Err(error) = run() {
        eprintln!("advisor: {error}");
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
            let mut game = prepared.new_game();
            let config = search_config(&prepared);
            let snapshot = search(&mut game, config, |_| {});
            let model = view_model(&prepared, &game, snapshot);
            let colour = if prepared.options.plain {
                ColourMode::Never
            } else {
                ColourMode::Always
            };
            println!(
                "{}",
                render(
                    &model,
                    usize::from(prepared.options.width),
                    usize::from(prepared.options.height),
                    colour,
                )
            );
            Ok(())
        }
    }
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
) -> AdvisorViewModel {
    AdvisorViewModel {
        p1: view_side(prepared, game, PlayerId::P1),
        p2: view_side(prepared, game, PlayerId::P2),
        us: prepared.options.us,
        mode: match prepared.options.second_card {
            Some(slot) => format!("ONE-ROUND HEURISTIC · SECOND · OPP CARD {slot}"),
            None => "ONE-ROUND HEURISTIC · FIRST".to_owned(),
        },
        limitation: "strict supported draw; one-round heuristic; hidden bets sampled uniformly"
            .to_owned(),
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
        label: if prepared.options.using_demo_draw {
            "DEMO".to_owned()
        } else {
            "CATALOG".to_owned()
        },
        life: position.players[player].life,
        pillz: position.players[player].pillz,
        cards,
    }
}

#[cfg(test)]
mod tests {
    use super::{search_config, AdvisorCommand};
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
}
