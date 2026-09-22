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
/// Every id here must be a real registry definition, or the family silently under-reports:
/// an id that no definition owns can never appear as a blocker. A family drops to zero once
/// its slice lands, which is the intended way to see that it is done.
const CANDIDATE_FAMILIES: &[(&str, &[u32])] = &[
    (
        "plain Heal N Max. M on the existing latch",
        &[649, 751, 963, 1501, 3118, 4625, 5341],
    ),
    (
        "plain Toxin N, Min M",
        &[1197, 1508, 1840, 4730, 5037, 5098, 5638, 5639, 5640],
    ),
    (
        "plain Poison N, Min M",
        &[206, 325, 509, 566, 582, 682, 1345, 1385, 3088, 3603, 5901],
    ),
    ("plain Regen N, Max. M", &[1458, 3433]),
    (
        "plain Toxin + Poison + Regen",
        &[
            206, 325, 509, 566, 582, 682, 1197, 1345, 1385, 1458, 1508, 1840, 3088, 3433, 3603,
            4730, 5037, 5098, 5638, 5639, 5640, 5901,
        ],
    ),
    (
        "plain Toxin + Poison + Regen with Symmetry/Asymmetry/Revenge forms",
        &[
            206, 325, 509, 566, 582, 682, 1197, 1345, 1385, 1458, 1508, 1840, 3088, 3301, 3433,
            3603, 4730, 5037, 5092, 5098, 5638, 5639, 5640, 5693, 5901,
        ],
    ),
    (
        "permanent Life (Poison/Heal/Toxin/Regen)",
        &[
            206, 325, 509, 566, 582, 649, 682, 751, 898, 963, 1197, 1266, 1282, 1345, 1385, 1458,
            1501, 1508, 1625, 1790, 1840, 2497, 3088, 3118, 3301, 3433, 3526, 3603, 4033, 4124,
            4210, 4561, 4625, 4730, 5037, 5092, 5098, 5316, 5341, 5578, 5594, 5613, 5638, 5639,
            5640, 5692, 5693, 5901,
        ],
    ),
    (
        "Life per Damage",
        &[141, 189, 226, 492, 1125, 1146, 1161, 1224, 4500],
    ),
    (
        "uncapped Life per Damage with Revenge/Confidence forms",
        &[141, 189, 226, 492, 1125, 1224, 1661, 1810, 4500],
    ),
    (
        "predicate-carrying plain permanents (Symmetry/Asymmetry/Revenge)",
        &[3301, 5092, 5692, 5693],
    ),
    (
        "uncapped Life per Damage + predicate-carrying plain permanents",
        &[
            141, 189, 226, 492, 1125, 1224, 1661, 1810, 3301, 4500, 5092, 5692, 5693,
        ],
    ),
    (
        "permanent Life + Life per Damage",
        &[
            141, 189, 206, 226, 325, 492, 509, 566, 582, 649, 682, 751, 898, 963, 1125, 1146, 1161,
            1197, 1224, 1266, 1282, 1345, 1385, 1458, 1501, 1508, 1625, 1790, 1840, 2497, 3088,
            3118, 3301, 3433, 3526, 3603, 4033, 4124, 4210, 4500, 4561, 4625, 4730, 5037, 5092,
            5098, 5316, 5341, 5578, 5594, 5613, 5638, 5639, 5640, 5692, 5693, 5901,
        ],
    ),
    (
        "own fixed Pillz",
        &[337, 455, 503, 1054, 1150, 1229, 2262, 2525, 4855, 5258],
    ),
    (
        "opposing Pillz reduction",
        &[334, 339, 343, 360, 570, 854, 3541, 5532, 5682],
    ),
    ("Pillz per Damage", &[809, 1051, 1090, 1852]),
    (
        "the three Pillz families together",
        &[
            334, 337, 339, 343, 360, 455, 503, 570, 809, 854, 1051, 1054, 1090, 1150, 1229, 1852,
            2262, 2525, 3541, 4855, 5258, 5532, 5682,
        ],
    ),
    ("Attack per opposing Power", &[1719, 1785, 4661]),
    ("conditional Copy: Unison", &[3994, 4141, 4767, 5073, 5108]),
    ("conditional stat Copy", &[1409, 4126, 4956]),
    ("Power/Damage Exchange", &[1588, 1592]),
    ("conditional Victory opponent-Life", &[1730, 4533]),
    (
        "plain Victory -N Opp. Life, Min M grammar",
        &[
            512, 524, 594, 602, 680, 769, 842, 935, 1002, 1399, 3491, 3571, 3716, 4948,
        ],
    ),
    (
        "Victory Or Defeat -N Opp. Life, Min M grammar",
        &[1386, 1628, 1726, 3367, 4331],
    ),
    (
        "Victory + Victory-or-Defeat opponent-Life reduction",
        &[
            512, 524, 594, 602, 680, 769, 842, 935, 1002, 1386, 1399, 1628, 1726, 3367, 3491, 3571,
            3716, 4331, 4948,
        ],
    ),
    (
        "predicate-carrying fixed Victory Life",
        &[814, 2113, 2638, 3546],
    ),
    ("capped Life per Damage", &[1146, 1161]),
    (
        "opponent-Life reduction + predicate-carrying Victory Life",
        &[
            512, 524, 594, 602, 680, 769, 814, 842, 935, 1002, 1386, 1399, 1628, 1726, 2113, 2638,
            3367, 3491, 3546, 3571, 3716, 4331, 4948,
        ],
    ),
    (
        "predicate-carrying fixed Victory Life + capped Life per Damage",
        &[814, 1146, 1161, 2113, 2638, 3546],
    ),
    (
        "opponent-Life reduction + predicate Victory Life + capped Life per Damage",
        &[
            512, 524, 594, 602, 680, 769, 814, 842, 935, 1002, 1146, 1161, 1386, 1399, 1628, 1726,
            2113, 2638, 3367, 3491, 3546, 3571, 3716, 4331, 4948,
        ],
    ),
    ("Xantiax both-sides Life reduction", &[1379, 5198]),
    ("Confidence: +N Pillz", &[1702]),
    ("Killshot: +N Pillz", &[2250, 4645]),
    (
        "prefixed +N Pillz (Confidence + Killshot)",
        &[1702, 2250, 4449, 4645],
    ),
    (
        "wider permanent Life (Defeat/Growth Poison)",
        &[1266, 1282, 4561],
    ),
    ("Courage: -4 Opp. Dmg, Min 2", &[304, 961]),
    ("Defeat: -2 Opp. Pillz, Min 4", &[912]),
    (
        "Courage opponent Damage + Defeat opponent Pillz",
        &[304, 912, 961],
    ),
    // The Killshot families the 2026-09-20 Dojo captures pin. `1204` fires in 1337321/1 and
    // pointedly does not in 1337230/0, where the same card wins without doubling; `1768`
    // fires in 1337321/2 with the server naming both halves in `postRoundAbilities`.
    (
        "Killshot opponent Life",
        &[1204, 1670, 1779, 1959, 4459, 4785, 5461, 5530],
    ),
    ("Killshot +N Pillz And Life", &[1768, 3106, 3318]),
    (
        "Killshot opponent Life + Pillz And Life",
        &[
            1204, 1670, 1768, 1779, 1959, 3106, 3318, 4459, 4785, 5461, 5530,
        ],
    ),
    (
        "Killshot opponent Life + own Pillz",
        &[1204, 1670, 1779, 1959, 2250, 4459, 4645, 4785, 5461, 5530],
    ),
    (
        "Killshot: +N Pillz with the third printed level",
        &[2250, 4311, 4645],
    ),
    ("Killshot own Life gain", &[1231, 2956, 3760, 5065, 5066]),
    (
        "every Killshot grammar (the whole sureshot channel)",
        &[
            1204, 1231, 1670, 1768, 1779, 1959, 2250, 2497, 2956, 3106, 3318, 3480, 3760, 3894,
            4311, 4459, 4645, 4785, 5065, 5066, 5461, 5530, 5775, 5776,
        ],
    ),
    // Candidates priced on 2026-09-21. Each is measured as its own line before anything is
    // admitted, and the combined lines are measured too, because neither additivity nor
    // overlap can be assumed.
    //
    // The conditional Victory opponent-Life line used to read `[1730, 4533]`, which was two
    // different grammars under one name and under-reported both. Courage is four printed
    // levels across two cards; Growth is a round-scaled magnitude and its own line.
    ("Courage Victory opponent Life", &[3314, 4531, 4532, 4533]),
    ("Growth Victory opponent Life", &[1730]),
    // The permanent-Life latch split by trigger. `Defeat : Heal` rides the same new
    // losing-round latch as `Defeat: Poison` and costs one extra text arm.
    (
        "Defeat permanent Life (Poison + Heal)",
        &[898, 1625, 1790, 4561],
    ),
    ("Defeat Poison alone", &[4561]),
    ("Growth permanent Life Poison", &[1266, 1282]),
    // Brawl is an anti-support magnitude, not a round counter. The magnitude channel is
    // nearly all of the cost and is paid once, so the whole combat-stat set is measured
    // beside its narrowest member.
    ("Brawl Power And Damage", &[1488, 1759, 5339, 5340]),
    (
        "every combat-stat Brawl grammar",
        &[
            1488, 1490, 1556, 1703, 1707, 1759, 1834, 2560, 2859, 2905, 2917, 2973, 3047, 3219,
            3272, 3303, 3855, 3936, 3948, 4463, 4826, 4897, 5255, 5339, 5340, 5376, 5497, 5519,
            5524, 5527, 5759,
        ],
    ),
    // The Oculus infiltration clan gate, narrowed to the members whose body is already an
    // admitted grammar, so the line prices the gate itself rather than the bodies.
    (
        "Oculus infiltration clan gate (already-bodied members)",
        &[
            2931, 4667, 4672, 4680, 5299, 5353, 5603, 5604, 5619, 5814, 5906, 5908, 5909, 5911,
            5912,
        ],
    ),
    // The clan gate narrowed again, to only those members whose body is a PLAIN numeric
    // grammar - no Courage position, no Growth/Degrowth scaling, no Equalizer, no Stop.
    // Those could be admitted by one orthogonal classifier that touches no existing
    // `neutral_except_*` gate, which is much the safer shape of the slice.
    (
        "Oculus clan gate (plain numeric bodies only)",
        &[4667, 5353, 5814, 5909, 5911, 5912],
    ),
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
    // What the cheapest remaining draws actually need. A family is worth proposing
    // only when it covers one of these sets whole; anything else merely co-occurs.
    {
        let mut by_set: BTreeMap<Vec<u32>, Vec<u64>> = BTreeMap::new();
        for (capture, blockers) in &per_capture {
            by_set
                .entry(blockers.iter().copied().collect())
                .or_default()
                .push(*capture);
        }
        let mut sets: Vec<_> = by_set.iter().collect();
        sets.sort_by_key(|(ids, captures)| (ids.len(), std::cmp::Reverse(captures.len())));
        println!("\n-- blocker sets, smallest first --");
        for (ids, captures) in sets.iter().take(12) {
            let described: Vec<String> = ids
                .iter()
                .map(|id| {
                    format!(
                        "{id} {}",
                        reach.get(id).map(|(d, _)| d.as_str()).unwrap_or("?")
                    )
                })
                .collect();
            println!(
                "  {} draw(s) need [{}]  e.g. {:?}",
                captures.len(),
                described.join(" | "),
                &captures[..captures.len().min(3)]
            );
        }
    }

    // The shape of the work left. Unlock ranks a family on its own; this ranks the corpus.
    // Admitting sources in reach order, how many draws come free at each step? A curve that
    // rises in steps says the mass is concentrated and one big slice buys a lot; a curve
    // that creeps says every draw needs its own long tail and slicing by unlock is right.
    {
        println!(
            "
-- cumulative: draws unlocked if the top-k sources by reach were all admitted --"
        );
        let order: Vec<u32> = ranked.iter().map(|(id, _)| **id).collect();
        let mut admitted: BTreeSet<u32> = BTreeSet::new();
        let mut last = 0usize;
        for (k, id) in order.iter().enumerate() {
            admitted.insert(*id);
            let unlocked = per_capture
                .values()
                .filter(|blockers| blockers.iter().all(|b| admitted.contains(b)))
                .count();
            let step = unlocked - last;
            if step > 0 || (k + 1) % 20 == 0 || k + 1 == order.len() {
                println!(
                    "  top {:3} -> {:3} of {} blocked draws (+{step:2})  {:5} {}",
                    k + 1,
                    unlocked,
                    per_capture.len(),
                    id,
                    reach.get(id).map(|(d, _)| d.as_str()).unwrap_or("?"),
                );
            }
            last = unlocked;
        }
        let mut per_draw: BTreeMap<usize, usize> = BTreeMap::new();
        for blockers in per_capture.values() {
            *per_draw.entry(blockers.len()).or_default() += 1;
        }
        println!("  blockers per blocked draw: {per_draw:?}");
        let sources: usize = per_capture.values().map(BTreeSet::len).sum();
        println!(
            "  unsupported slots {sources} of {} scanned card slots ({:.1}% of cards executable)",
            scanned * 8,
            100.0 - (sources as f64) * 100.0 / (scanned * 8) as f64,
        );
        println!("  blocker ids: {order:?}");
        println!("  distinct blocking sources: {}", order.len());
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
