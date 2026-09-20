//! Measurement for per-decision admission: how much would the advisor cover if a draw no
//! longer had to be understood whole?
//!
//! `CatalogCombatStatMatchV1` accepts a draw only when all eight cards are executable, which
//! is one game in five. But a live decision in round `r` only explores the cards that are
//! still in hand: the cards already played are history, and the advisor reconciles life and
//! pillz against the server's own snapshot rather than recomputing them. So a decision can
//! be sound while the draw as a whole is not - provided nothing an already-played card did
//! is still paying out. That last part is the catch, and it is why this is a report and not
//! a change: a latched permanent (Heal, Poison, Toxin, Regen and their prefixed forms) keeps
//! paying in every later round, so an unsupported card that latched one is not history at
//! all.
//!
//! Three numbers, deliberately bracketing the honest answer:
//!
//! * `whole draw` - today's rule, counted in decisions rather than draws.
//! * `remaining cards` - every card still in either hand is executable. This is the
//!   optimistic bound: it assumes everything already played is finished paying.
//! * `remaining, no persistent history` - the same, minus every decision where an
//!   already-played card prints text that could still be paying. That keyword scan is a
//!   measurement heuristic, not a semantic rule: it is deliberately generous about what
//!   counts as persistent, so the number it produces is the conservative one.
//!
//! ```text
//! cargo test --manifest-path rust/Cargo.toml --locked \
//!     --test partial_admission_report -- --ignored --nocapture
//! ```
use std::collections::BTreeMap;
use std::path::PathBuf;

use urban_recreation_rust::catalog::{CardKey, EffectiveCardCatalog};
use urban_recreation_rust::effect_registry::EffectRegistryV1;
use urban_recreation_rust::engine::{
    ByPlayer, CatalogCombatStatMatchInputV1, CatalogCombatStatMatchV1,
    CatalogCombatStatPlayerInputV1, CatalogCombatStatProjectionV1,
};
use urban_recreation_rust::replay::{load_corpus, EnginePlayer, ReplayCaseV1};

const PROJECTION: CatalogCombatStatProjectionV1 =
    CatalogCombatStatProjectionV1::RequireFullyExecutableDraws;

/// Neutral stand-ins with no active source of their own, one per slot so that retiring
/// several slots never trips the duplicate-character rejection.
const FILLER: [CardKey; 8] = [
    CardKey { id: 123, level: 1 },
    CardKey { id: 124, level: 1 },
    CardKey { id: 138, level: 1 },
    CardKey { id: 139, level: 1 },
    CardKey { id: 441, level: 1 },
    CardKey { id: 444, level: 1 },
    CardKey { id: 445, level: 1 },
    CardKey { id: 447, level: 1 },
];

/// Text that can still be paying out in a later round. Kept generous on purpose: this scan
/// is asking which decisions are safe, so anything arguable counts as unsafe.
const PERSISTENT_TEXT: &[&str] = &[
    "Heal",
    "Poison",
    "Toxin",
    "Regen",
    "Infection",
    "Consume",
    "Repair",
    "Combust",
    "Dope",
    "Mindwipe",
    "Growth",
    "Degrowth",
    "Backlash",
    "Xantiax",
    "Reanimate",
    "Unison",
];

fn root_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(path)
}

fn catalog() -> EffectiveCardCatalog {
    EffectiveCardCatalog::load(
        root_path("data/data.json"),
        root_path("data/battle_card_overrides.json"),
    )
    .unwrap()
}

fn registry() -> EffectRegistryV1 {
    EffectRegistryV1::load(root_path("captures/abilities.json")).unwrap()
}

fn player_index(player: EnginePlayer) -> usize {
    match player {
        EnginePlayer::P1 => 0,
        EnginePlayer::P2 => 1,
    }
}

/// Build the draw as it stands at the decision in `round`, with every slot either side has
/// already played replaced by a neutral filler.
fn constructs(
    case: &ReplayCaseV1,
    round: usize,
    catalog: &EffectiveCardCatalog,
    registry: &EffectRegistryV1,
    retire: bool,
) -> bool {
    let mut played = [[false; 4]; 2];
    if retire {
        for earlier in &case.rounds[..round] {
            for play in &earlier.plays {
                played[player_index(play.engine_player)][usize::from(play.hand_index)] = true;
            }
        }
    }
    let make = |side: usize| CatalogCombatStatPlayerInputV1 {
        initial_life: case.players[side].base_life,
        initial_pillz: case.players[side].base_pillz,
        hand: std::array::from_fn(|slot| {
            if played[side][slot] {
                FILLER[side * 4 + slot]
            } else {
                case.players[side].hand[slot].key
            }
        }),
    };
    let input = CatalogCombatStatMatchInputV1 {
        battle_rule_id: case.metadata.battle_rule_id,
        night: case.metadata.night,
        players: ByPlayer::new(make(0), make(1)),
    };
    CatalogCombatStatMatchV1::new(input, catalog, registry, PROJECTION).is_ok()
}

/// Whether a slot either side has already played prints text that could still be paying.
fn persistent_history(case: &ReplayCaseV1, round: usize) -> bool {
    for earlier in &case.rounds[..round] {
        for play in &earlier.plays {
            let card =
                &case.players[player_index(play.engine_player)].hand[usize::from(play.hand_index)];
            for source in [&card.source_ability, &card.source_bonus] {
                let Some(source) = source else { continue };
                if PERSISTENT_TEXT
                    .iter()
                    .any(|text| source.description.contains(text))
                {
                    return true;
                }
            }
        }
    }
    false
}

#[test]
#[ignore = "report, not a gate"]
fn per_decision_admission_measures_what_partial_coverage_would_buy() {
    let catalog = catalog();
    let registry = registry();
    let corpus = load_corpus(root_path("captures/games"), root_path("data/data.json")).unwrap();

    let mut decisions = 0_usize;
    let mut whole_draw = 0_usize;
    let mut remaining = 0_usize;
    let mut remaining_clean = 0_usize;
    let mut by_round: BTreeMap<usize, (usize, usize, usize)> = BTreeMap::new();
    let mut newly_reachable_draws = 0_usize;

    for case in &corpus.ready {
        let whole = constructs(case, 0, &catalog, &registry, false);
        let mut gained = false;
        for round in 0..case.rounds.len() {
            decisions += 1;
            let entry = by_round.entry(round + 1).or_insert((0, 0, 0));
            entry.0 += 1;
            if whole {
                whole_draw += 1;
                remaining += 1;
                remaining_clean += 1;
                entry.1 += 1;
                entry.2 += 1;
                continue;
            }
            if !constructs(case, round, &catalog, &registry, true) {
                continue;
            }
            remaining += 1;
            entry.1 += 1;
            gained = true;
            if !persistent_history(case, round) {
                remaining_clean += 1;
                entry.2 += 1;
            }
        }
        if gained {
            newly_reachable_draws += 1;
        }
    }

    let percent = |part: usize| 100.0 * part as f64 / decisions as f64;
    println!("replayable draws: {}", corpus.ready.len());
    println!("decision points: {decisions}");
    println!(
        "  whole draw (today):            {whole_draw:4} ({:.1}%)",
        percent(whole_draw)
    );
    println!(
        "  remaining cards:               {remaining:4} ({:.1}%)",
        percent(remaining)
    );
    println!(
        "  remaining, no persistent past: {remaining_clean:4} ({:.1}%)",
        percent(remaining_clean)
    );
    println!("draws that gain at least one decision: {newly_reachable_draws}");
    println!("round  total  remaining  clean");
    for (round, (total, gained, clean)) in by_round {
        println!("{round:5}  {total:5}  {gained:9}  {clean:5}");
    }
}
