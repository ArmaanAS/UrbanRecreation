//! Measurement tool for choosing the next strict-catalog slice.
//!
//! `CatalogCombatStatMatchV1` rejects a whole draw at the first unsupported source, so a
//! single error says nothing about how much a family actually costs. This scan replaces
//! each blocking slot with a neutral filler and retries, collecting every blocker in a
//! draw, then reports two different things: how many draws a source appears in (reach),
//! and how many draws a family would make eligible on its own (unlock). Those rank very
//! differently, and unlock is the one worth acting on.
//!
//! It is ignored by default because it is a report rather than a gate; the pinned
//! eligibility set lives in `catalog_match.rs`. Run it with:
//!
//! ```text
//! cargo test --manifest-path rust/Cargo.toml --locked \
//!     --test strict_coverage_blockers -- --ignored --nocapture
//! ```
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use urban_recreation_rust::catalog::{CardKey, EffectiveCardCatalog};
use urban_recreation_rust::effect_registry::EffectRegistryV1;
use urban_recreation_rust::engine::{
    ByPlayer, CatalogCombatStatMatchErrorV1, CatalogCombatStatMatchInputV1,
    CatalogCombatStatMatchV1, CatalogCombatStatPlayerInputV1, CatalogCombatStatProjectionV1,
    PlayerId,
};

const PROJECTION: CatalogCombatStatProjectionV1 =
    CatalogCombatStatProjectionV1::RequireFullyExecutableDraws;

/// Neutral stand-ins with no active source of their own, used to retire a blocking slot so
/// the scan can see what else the draw would need.
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

/// Candidate slices, each as the registry definition ids it would admit. A draw counts as
/// unlocked by a family when every one of its blockers is in that family.
const CANDIDATE_FAMILIES: &[(&str, &[u32])] = &[
    ("conditional Copy: Reprisal/Revenge", &[958, 990]),
    ("conditional Copy: Asymmetry/Unison", &[2482]),
    ("stat Copy: Power/Damage/exchange", &[315, 1592, 1588]),
    (
        "conditional Victory opponent-Life",
        &[4708, 4533, 3016, 1730],
    ),
    ("Protection", &[759, 461, 481, 1132, 1355]),
];

fn root_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(path)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Capture {
    id: u64,
    battle_rule_id: u32,
    night: bool,
    players: Vec<Player>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Player {
    base_life: u16,
    base_pillz: u16,
    hand: Vec<Card>,
}

#[derive(serde::Deserialize)]
struct Card {
    id: u32,
    level: u8,
    index: u8,
}

struct Blocker {
    registry_definition_id: u32,
    description: String,
}

/// Collect every source standing between this draw and strict eligibility, or `None` when
/// the draw is refused structurally rather than for a missing effect slice.
fn blockers_for(
    capture: &Capture,
    catalog: &EffectiveCardCatalog,
    registry: &EffectRegistryV1,
) -> Option<Vec<Blocker>> {
    let mut hands: [[CardKey; 4]; 2] = std::array::from_fn(|side| {
        std::array::from_fn(|index| {
            let card = capture.players[side]
                .hand
                .iter()
                .find(|card| usize::from(card.index) == index)
                .expect("a complete hand covers every index");
            CardKey::new(card.id, card.level)
        })
    });
    let mut blockers: Vec<Blocker> = Vec::new();
    while blockers.len() <= FILLER.len() {
        let input = CatalogCombatStatMatchInputV1 {
            battle_rule_id: capture.battle_rule_id,
            night: capture.night,
            players: ByPlayer::new(
                CatalogCombatStatPlayerInputV1 {
                    initial_life: capture.players[0].base_life,
                    initial_pillz: capture.players[0].base_pillz,
                    hand: hands[0],
                },
                CatalogCombatStatPlayerInputV1 {
                    initial_life: capture.players[1].base_life,
                    initial_pillz: capture.players[1].base_pillz,
                    hand: hands[1],
                },
            ),
        };
        match CatalogCombatStatMatchV1::new(input, catalog, registry, PROJECTION) {
            Ok(_) => return Some(blockers),
            Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
                player,
                hand_slot,
                description,
                registry_definition_id,
                ..
            }) => {
                let side = if player == PlayerId::P1 { 0 } else { 1 };
                hands[side][usize::from(hand_slot.get())] = FILLER[blockers.len()];
                blockers.push(Blocker {
                    registry_definition_id,
                    description,
                });
            }
            // A Leader, duplicate character, or lookup failure is not a missing effect
            // slice, so it ends the walk rather than being attributed to some family.
            Err(_) => return None,
        }
    }
    None
}

#[test]
#[ignore = "measurement report for planning the next slice, not a gate"]
fn report_strict_coverage_blockers() {
    let catalog = EffectiveCardCatalog::load(
        root_path("data/data.json"),
        root_path("data/battle_card_overrides.json"),
    )
    .unwrap();
    let registry = EffectRegistryV1::load(root_path("captures/abilities.json")).unwrap();
    let mut paths = fs::read_dir(root_path("captures/games"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    paths.sort();

    let mut scanned = 0_usize;
    let mut eligible = 0_usize;
    let mut structural = 0_usize;
    let mut reach: BTreeMap<u32, (String, BTreeSet<u64>)> = BTreeMap::new();
    let mut per_capture: BTreeMap<u64, BTreeSet<u32>> = BTreeMap::new();

    for path in paths {
        let capture: Capture = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        if capture.players.len() != 2 || capture.players.iter().any(|player| player.hand.len() != 4)
        {
            continue;
        }
        scanned += 1;
        let Some(blockers) = blockers_for(&capture, &catalog, &registry) else {
            structural += 1;
            continue;
        };
        if blockers.is_empty() {
            eligible += 1;
            continue;
        }
        for blocker in &blockers {
            reach
                .entry(blocker.registry_definition_id)
                .or_insert_with(|| (blocker.description.clone(), BTreeSet::new()))
                .1
                .insert(capture.id);
            per_capture
                .entry(capture.id)
                .or_default()
                .insert(blocker.registry_definition_id);
        }
    }

    println!(
        "scanned {scanned} complete draws: {eligible} eligible, {structural} structurally refused"
    );
    println!("\n-- reach: draws a source appears in (not what admitting it would unlock) --");
    let mut ranked: Vec<_> = reach.iter().collect();
    ranked.sort_by_key(|(id, (_, ids))| (std::cmp::Reverse(ids.len()), **id));
    for (id, (description, ids)) in ranked.iter().take(15) {
        println!("{:4}  {id:5}  {description}", ids.len());
    }
    println!("\n-- unlock: draws whose every remaining blocker is in the family --");
    for (name, ids) in CANDIDATE_FAMILIES {
        let unlocked = per_capture
            .values()
            .filter(|blockers| blockers.iter().all(|id| ids.contains(id)))
            .count();
        println!("{unlocked:4}  {name}");
    }

    // The scan stays total and attributable: every draw is eligible, structurally refused,
    // or carries at least one blocker naming a real registry definition.
    assert_eq!(
        scanned,
        eligible + structural + per_capture.len(),
        "every scanned draw must be accounted for exactly once",
    );
    for (id, blockers) in &per_capture {
        assert!(
            !blockers.is_empty(),
            "capture {id} was recorded without a blocker",
        );
    }
    for (id, (description, _)) in &reach {
        assert!(
            *id != 0 && !description.is_empty(),
            "a blocker must name a real registry definition",
        );
    }
}
