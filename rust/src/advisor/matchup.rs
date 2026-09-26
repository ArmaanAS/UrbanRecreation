//! Batch hand-versus-hand evaluation for deck building (phase 5 of
//! `docs/deck-builder-design.md`).
//!
//! A `solve` is exactly the advisor's round-one FIRST decision for a draw prepared from card
//! keys: [`prepare_with_sources`] builds the strict `CatalogCombatStatMatchV1`, and
//! [`search_with_threads`] ranks every opening move with the exact conservative continuation
//! while weighting the opponent's reply by the captured opening prior. The value reported is
//! the advisor's top-ranked move's weighted average, in the first mover's frame. A draw the
//! strict catalog refuses is reported as refused, never scored by a guess.
//!
//! A `probe` asks whether one card can sit in a strictly executable draw in a neutral context,
//! which is an approximation made deliberately simple:
//! - its ability: the card plus three level-1 no-ability fillers of three other clans, so no
//!   clan bonus is live, against four such fillers;
//! - its clan bonus: the card, one clan-mate (the first, in a fixed order that tries
//!   no-ability rows first, whose own ability is not what refuses the draw) and two fillers.
//!
//! A card whose ability refuses only against particular opposing cards, or only with more
//! clan-mates than one, can still pass a probe; a solve on the real draw stays the authority.
//!
//! The wire is JSONL: one request per line, one response per request line, in request order.
//! Blank lines are skipped. Requests run in parallel on a pool of `threads` workers, each solve
//! single-threaded (its continuation cache is per solve, so solving several draws at once is
//! the efficient split), and every response is a pure function of its request apart from `ms`.

use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::num::NonZeroUsize;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::advisor::input::{
    prepare_with_sources, repository_root, AdvisorInputError, AdvisorOptions, BATTLE_RULE_ID,
};
use crate::advisor::search::{
    search_with_threads, AdvisorMove, SearchConfig, SearchMode, ADVISOR_POLICY_SEMANTIC_REVISION_V1,
};
use crate::catalog::{CardKey, EffectiveCardCatalog};
use crate::effect_registry::EffectRegistryV1;
use crate::engine::combat_stat_compiler::COMBAT_STAT_COMPILER_POLICY_SEMANTIC_REVISION_V1;
use crate::engine::{
    derive_catalog_hand, ByPlayer, CatalogCombatStatMatchErrorV1, CatalogCombatStatMatchInputV1,
    CatalogCombatStatMatchV1, CatalogCombatStatPlayerInputV1, CatalogCombatStatProjectionV1,
    CombatStatEffectSourceV1, CombatStatPlanErrorV1, EffectiveCatalogHandErrorV1, PlayerId,
    CATALOG_CONTEXT_POLICY_SEMANTIC_REVISION_V1, HAND_SIZE, LEADER_CLAN_ID, OCULUS_CLAN_ID,
};

/// Wire and semantics of this module. Bump it when a response field or the meaning of a
/// value changes, so a host's result cache keyed on it is invalidated.
pub const MATCHUP_PROTOCOL_VERSION: u16 = 1;
/// A normal room: 12 Life and 12 Pillz a side (every captured room so far).
pub const MATCHUP_LIFE: u16 = 12;
pub const MATCHUP_PILLZ: u16 = 12;
const MAX_LIFE: u16 = 255;
/// Every legal root action is searched, so this keeps a request's work predictable.
const MAX_PILLZ: u16 = 30;
const MAX_LINE_BYTES: usize = 16_384;
/// Clan-mates a bonus probe tries before calling the bonus untested.
const MAX_PARTNER_ATTEMPTS: usize = 24;
const NO_ABILITY: &str = "No Ability";

/// The repository inputs every request reads, loaded once per process.
pub struct MatchupSources {
    catalog: EffectiveCardCatalog,
    registry: EffectRegistryV1,
    /// Neutral stand-ins, one per clan, in ascending clan order.
    fillers: Vec<Filler>,
    /// Every catalog row of a clan, in ascending `(id, level)` order.
    clan_rows: BTreeMap<u32, Vec<CardKey>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Filler {
    key: CardKey,
    clan_id: u32,
}

impl MatchupSources {
    /// Loads `data/data.json`, `data/battle_card_overrides.json` and
    /// `captures/abilities.json` from the repository this binary was built in.
    pub fn load() -> Result<Self, String> {
        let root = repository_root();
        let catalog = EffectiveCardCatalog::load(
            root.join("data/data.json"),
            root.join("data/battle_card_overrides.json"),
        )
        .map_err(|error| format!("failed to load effective catalog: {error}"))?;
        let registry = EffectRegistryV1::load(root.join("captures/abilities.json"))
            .map_err(|error| format!("failed to load effect registry: {error}"))?;
        Self::from_parts(catalog, registry)
    }

    pub fn from_parts(
        catalog: EffectiveCardCatalog,
        registry: EffectRegistryV1,
    ) -> Result<Self, String> {
        let mut clan_rows: BTreeMap<u32, Vec<CardKey>> = BTreeMap::new();
        for (key, card) in catalog.iter() {
            clan_rows.entry(card.clan_id).or_default().push(key);
        }
        let mut sources = Self {
            catalog,
            registry,
            fillers: Vec::new(),
            clan_rows,
        };
        sources.fillers = sources.select_fillers()?;
        Ok(sources)
    }

    pub const fn catalog(&self) -> &EffectiveCardCatalog {
        &self.catalog
    }

    pub const fn registry(&self) -> &EffectRegistryV1 {
        &self.registry
    }

    pub fn filler_keys(&self) -> Vec<CardKey> {
        self.fillers.iter().map(|filler| filler.key).collect()
    }

    /// The lowest-id level-1 card of each clan with no ability by day or by night, kept only
    /// while every one of them passes both a day and a night draw made of the others.
    fn select_fillers(&self) -> Result<Vec<Filler>, String> {
        let mut by_clan: BTreeMap<u32, Filler> = BTreeMap::new();
        for (key, card) in self.catalog.iter() {
            let no_night_ability = card
                .night_ability
                .as_deref()
                .is_none_or(|text| text == NO_ABILITY);
            if key.level == 1
                && card.ability == NO_ABILITY
                && no_night_ability
                && card.clan_id != LEADER_CLAN_ID
                && card.clan_id != OCULUS_CLAN_ID
            {
                by_clan.entry(card.clan_id).or_insert(Filler {
                    key,
                    clan_id: card.clan_id,
                });
            }
        }
        let mut pool: Vec<Filler> = by_clan.into_values().collect();
        'retry: loop {
            if pool.len() < 8 {
                return Err(format!(
                    "only {} clans have a usable level-1 no-ability filler; need 8",
                    pool.len()
                ));
            }
            for (index, filler) in pool.iter().enumerate() {
                let mut hand = vec![filler.key];
                hand.extend(
                    pool.iter()
                        .filter(|other| other.clan_id != filler.clan_id)
                        .take(HAND_SIZE - 1)
                        .map(|other| other.key),
                );
                let hand: [CardKey; HAND_SIZE] = hand.try_into().expect("eight fillers");
                let opponent = self.opponent_fillers(&pool, filler.key.id);
                for night in [false, true] {
                    if let Err(error) = self.construct(hand, opponent, night) {
                        let culprit = match error_location(&error) {
                            Some((PlayerId::P1, slot, _)) => hand[slot],
                            Some((PlayerId::P2, slot, _)) => opponent[slot],
                            None => pool[index].key,
                        };
                        pool.retain(|candidate| candidate.key != culprit);
                        continue 'retry;
                    }
                }
            }
            return Ok(pool);
        }
    }

    fn opponent_fillers(&self, pool: &[Filler], avoid_id: u32) -> [CardKey; HAND_SIZE] {
        let keys: Vec<CardKey> = pool
            .iter()
            .rev()
            .filter(|filler| filler.key.id != avoid_id)
            .take(HAND_SIZE)
            .map(|filler| filler.key)
            .collect();
        keys.try_into().expect("at least five fillers")
    }

    /// Fillers for our own hand: distinct clans, none of `avoid_clans`, none of `avoid_ids`.
    fn own_fillers(&self, avoid_clans: &[u32], avoid_ids: &[u32], count: usize) -> Vec<CardKey> {
        self.fillers
            .iter()
            .filter(|filler| {
                !avoid_clans.contains(&filler.clan_id) && !avoid_ids.contains(&filler.key.id)
            })
            .take(count)
            .map(|filler| filler.key)
            .collect()
    }

    fn construct(
        &self,
        p1: [CardKey; HAND_SIZE],
        p2: [CardKey; HAND_SIZE],
        night: bool,
    ) -> Result<CatalogCombatStatMatchV1, CatalogCombatStatMatchErrorV1> {
        let player = |hand| CatalogCombatStatPlayerInputV1 {
            initial_life: MATCHUP_LIFE,
            initial_pillz: MATCHUP_PILLZ,
            hand,
        };
        CatalogCombatStatMatchV1::new(
            CatalogCombatStatMatchInputV1 {
                battle_rule_id: BATTLE_RULE_ID,
                night,
                players: ByPlayer::new(player(p1), player(p2)),
            },
            &self.catalog,
            &self.registry,
            CatalogCombatStatProjectionV1::RequireFullyExecutableDraws,
        )
    }
}

/// Where a refusal points: the player, the hand slot and, when known, which source.
fn error_location(
    error: &CatalogCombatStatMatchErrorV1,
) -> Option<(PlayerId, usize, Option<CombatStatEffectSourceV1>)> {
    use CatalogCombatStatMatchErrorV1 as E;
    match error {
        E::Hand { player, source } => match source {
            EffectiveCatalogHandErrorV1::MissingCard { hand_slot, .. }
            | EffectiveCatalogHandErrorV1::MissingClan { hand_slot, .. } => {
                Some((*player, hand_slot.index(), None))
            }
        },
        E::DuplicateCharacter {
            player,
            second_slot,
            ..
        } => Some((*player, second_slot.index(), None)),
        E::WholeHandLeaderHazard {
            player, hand_slot, ..
        } => Some((*player, hand_slot.index(), None)),
        E::Lookup {
            player,
            hand_slot,
            source_kind,
            ..
        }
        | E::UnsupportedSource {
            player,
            hand_slot,
            source_kind,
            ..
        }
        | E::UnsupportedCompiledShape {
            player,
            hand_slot,
            source_kind,
            ..
        } => Some((*player, hand_slot.index(), Some(*source_kind))),
        E::EnginePlan(plan) => match plan {
            CombatStatPlanErrorV1::CardMismatch(mismatch) => {
                Some((mismatch.player, mismatch.hand_slot.index(), None))
            }
            CombatStatPlanErrorV1::InvalidSourceBonusContext {
                player, hand_slot, ..
            } => Some((
                *player,
                hand_slot.index(),
                Some(CombatStatEffectSourceV1::Bonus),
            )),
            CombatStatPlanErrorV1::InvalidAbilitySupportContext {
                player, hand_slot, ..
            } => Some((
                *player,
                hand_slot.index(),
                Some(CombatStatEffectSourceV1::Ability),
            )),
            CombatStatPlanErrorV1::InvalidExecute {
                player,
                hand_slot,
                source,
                ..
            } => Some((*player, hand_slot.index(), Some(*source))),
        },
    }
}

/// One exact round-one decision to evaluate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SolveRequest {
    pub p1: [CardKey; HAND_SIZE],
    pub p2: [CardKey; HAND_SIZE],
    pub first_mover: PlayerId,
    pub night: bool,
    pub life: u16,
    pub pillz: u16,
    /// `None` searches to completion. A search the budget cuts is reported as incomplete,
    /// never as a partial value.
    pub budget: Option<Duration>,
}

impl SolveRequest {
    pub const fn new(
        p1: [CardKey; HAND_SIZE],
        p2: [CardKey; HAND_SIZE],
        first_mover: PlayerId,
    ) -> Self {
        Self {
            p1,
            p2,
            first_mover,
            night: false,
            life: MATCHUP_LIFE,
            pillz: MATCHUP_PILLZ,
            budget: None,
        }
    }
}

/// The advisor's recommended opening move and its value, in the first mover's frame.
#[derive(Clone, Debug, PartialEq)]
pub struct SolveResult {
    /// The top-ranked move's opening-prior-weighted average over the opponent's replies.
    pub value: f64,
    /// That move's worst complete reply.
    pub worst: f64,
    pub best: f64,
    pub best_move: AdvisorMove,
    /// Shares of that move's (unweighted) replies that knock the opponent out this round,
    /// and that knock the first mover out.
    pub ko_share: f64,
    pub koed_share: f64,
    pub root_moves: usize,
    pub replies: usize,
    pub elapsed: Duration,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SolveOutcome {
    Solved(SolveResult),
    /// The strict catalog would not build this draw; the reason is its own.
    Refused(String),
    Incomplete {
        elapsed: Duration,
    },
}

/// Solves one draw exactly as the advisor does its round-one FIRST decision, single-threaded.
pub fn solve(sources: &MatchupSources, request: &SolveRequest) -> SolveOutcome {
    let started = Instant::now();
    let options = AdvisorOptions {
        p1: request.p1,
        p2: request.p2,
        life: request.life,
        pillz: request.pillz,
        night: request.night,
        us: request.first_mover,
        first_mover: request.first_mover,
        using_demo_draw: false,
        ..AdvisorOptions::default()
    };
    let prepared = match prepare_with_sources(options, &sources.catalog, &sources.registry) {
        Ok(prepared) => prepared,
        Err(AdvisorInputError::UnsupportedDraw(error)) => {
            return SolveOutcome::Refused(error.to_string())
        }
        Err(error) => return SolveOutcome::Refused(error.to_string()),
    };
    let mut game = prepared.new_game();
    let config = SearchConfig {
        us: request.first_mover,
        first_mover: request.first_mover,
        mode: SearchMode::First,
        budget: request.budget.unwrap_or(Duration::MAX),
    };
    let snapshot = search_with_threads(&mut game, config, NonZeroUsize::MIN, |_| {});
    let elapsed = started.elapsed();
    let Some(top) = snapshot.ranked.first().filter(|_| snapshot.complete) else {
        return SolveOutcome::Incomplete { elapsed };
    };
    let share = |count: usize| {
        if top.samples == 0 {
            0.0
        } else {
            count as f64 / top.samples as f64
        }
    };
    SolveOutcome::Solved(SolveResult {
        value: top.average,
        worst: top.worst,
        best: top.best,
        best_move: top.move_,
        ko_share: share(top.kos),
        koed_share: share(top.koed),
        root_moves: snapshot.ranked.len(),
        replies: top.samples,
        elapsed,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProbeStatus {
    /// Executable alone and beside a clan-mate (its bonus live).
    Exact,
    /// Executable only without a clan-mate: the bonus, or the ability in a clan context,
    /// refuses.
    BonusRefused,
    /// The ability passes, but no clan-mate could be found to test the bonus with.
    BonusUntested,
    /// The card's ability (or the card itself) refuses even in the neutral draw.
    Refused,
    /// A Leader: the strict catalog refuses every hand holding one.
    Leader,
    /// This `(id, level)` is not in the catalog the engine loads.
    Missing,
}

impl ProbeStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::BonusRefused => "bonus_refused",
            Self::BonusUntested => "bonus_untested",
            Self::Refused => "refused",
            Self::Leader => "leader",
            Self::Missing => "missing",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProbeCheck {
    Passed {
        partner: Option<CardKey>,
    },
    Refused {
        reason: String,
        partner: Option<CardKey>,
    },
    Untested {
        reason: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeResult {
    pub status: ProbeStatus,
    pub ability: ProbeCheck,
    pub bonus: ProbeCheck,
    /// The ability and bonus text the engine would use for this card in that context.
    pub ability_text: Option<String>,
    pub bonus_text: Option<String>,
}

/// Whether `card` can appear in a strictly executable draw in a neutral context; see the
/// module documentation for exactly which draws are tried.
pub fn probe(sources: &MatchupSources, card: CardKey, night: bool) -> ProbeResult {
    let Some(row) = sources.catalog.get(card) else {
        let reason = format!(
            "card id {} level {} is not in data/data.json",
            card.id, card.level
        );
        return ProbeResult {
            status: ProbeStatus::Missing,
            ability: ProbeCheck::Refused {
                reason,
                partner: None,
            },
            bonus: ProbeCheck::Untested {
                reason: "the card is missing".to_owned(),
            },
            ability_text: None,
            bonus_text: None,
        };
    };
    let clan_id = row.clan_id;
    let own = sources.own_fillers(&[clan_id], &[card.id], HAND_SIZE - 1);
    let hand = [card, own[0], own[1], own[2]];
    let opponent = sources.opponent_fillers(&sources.fillers, card.id);
    let ability_text = derive_catalog_hand(hand, night, &sources.catalog)
        .ok()
        .and_then(|derived| derived[0].ability.as_ref().map(|a| a.description.clone()));
    if let Err(error) = sources.construct(hand, opponent, night) {
        let status = match error {
            CatalogCombatStatMatchErrorV1::WholeHandLeaderHazard { .. } => ProbeStatus::Leader,
            CatalogCombatStatMatchErrorV1::Hand { .. } => ProbeStatus::Missing,
            _ => ProbeStatus::Refused,
        };
        return ProbeResult {
            status,
            ability: ProbeCheck::Refused {
                reason: error.to_string(),
                partner: None,
            },
            bonus: ProbeCheck::Untested {
                reason: "the card refuses without a clan-mate".to_owned(),
            },
            ability_text,
            bonus_text: None,
        };
    }

    let ability = ProbeCheck::Passed { partner: None };
    let mates = sources
        .clan_rows
        .get(&clan_id)
        .map_or(&[][..], Vec::as_slice);
    let has_no_ability = |key: &CardKey| {
        sources.catalog.get(*key).is_some_and(|mate| {
            let night_text = night.then_some(()).and(mate.night_ability.as_deref());
            night_text.unwrap_or(&mate.ability) == NO_ABILITY
        })
    };
    let candidates = mates
        .iter()
        .filter(|mate| mate.id != card.id && has_no_ability(mate))
        .chain(
            mates
                .iter()
                .filter(|mate| mate.id != card.id && !has_no_ability(mate)),
        )
        .take(MAX_PARTNER_ATTEMPTS);
    let mut tried = 0;
    for &partner in candidates {
        tried += 1;
        let own = sources.own_fillers(&[clan_id], &[card.id, partner.id], HAND_SIZE - 2);
        let hand = [card, partner, own[0], own[1]];
        let bonus_text = derive_catalog_hand(hand, night, &sources.catalog)
            .ok()
            .and_then(|derived| derived[0].bonus.as_ref().map(|b| b.description.clone()));
        match sources.construct(hand, opponent, night) {
            Ok(_) => {
                return ProbeResult {
                    status: ProbeStatus::Exact,
                    ability,
                    bonus: ProbeCheck::Passed {
                        partner: Some(partner),
                    },
                    ability_text,
                    bonus_text,
                }
            }
            // The partner's own ability is not this card's problem: try another clan-mate.
            Err(error)
                if error_location(&error)
                    == Some((PlayerId::P1, 1, Some(CombatStatEffectSourceV1::Ability))) => {}
            Err(error) => {
                return ProbeResult {
                    status: ProbeStatus::BonusRefused,
                    ability,
                    bonus: ProbeCheck::Refused {
                        reason: error.to_string(),
                        partner: Some(partner),
                    },
                    ability_text,
                    bonus_text,
                }
            }
        }
    }
    ProbeResult {
        status: ProbeStatus::BonusUntested,
        ability,
        bonus: ProbeCheck::Untested {
            reason: if tried == 0 {
                "no other card of this clan is in the catalog".to_owned()
            } else {
                format!("none of the {tried} clan-mates tried passes its own ability")
            },
        },
        ability_text,
        bonus_text: None,
    }
}

fn key_json(key: CardKey) -> Value {
    json!([key.id, key.level])
}

fn check_json(check: &ProbeCheck) -> Value {
    let mut object = Map::new();
    let (ok, reason, partner) = match check {
        ProbeCheck::Passed { partner } => (json!(true), None, *partner),
        ProbeCheck::Refused { reason, partner } => (json!(false), Some(reason), *partner),
        ProbeCheck::Untested { reason } => (Value::Null, Some(reason), None),
    };
    object.insert("ok".to_owned(), ok);
    if let Some(reason) = reason {
        object.insert("reason".to_owned(), json!(reason));
    }
    if let Some(partner) = partner {
        object.insert("partner".to_owned(), key_json(partner));
    }
    Value::Object(object)
}

fn move_json(move_: AdvisorMove) -> Value {
    json!({
        "hand_index": move_.hand_index,
        "pillz": move_.pillz,
        "fury": move_.fury,
    })
}

/// The data fingerprints and semantic revisions every response depends on, in the same
/// spelling as the advisor worker's V3 provenance.
pub fn provenance_json(sources: &MatchupSources) -> Value {
    json!({
        "protocol": MATCHUP_PROTOCOL_VERSION,
        "effective_catalog_fingerprint_fnv1a64":
            format!("{:016x}", sources.catalog.source_fingerprint_fnv1a64().value()),
        "effect_registry_fingerprint_fnv1a64":
            format!("{:016x}", sources.registry.source_fingerprint_fnv1a64().value()),
        "effect_registry_schema_version": sources.registry.schema_version(),
        "compiler_policy_semantic_revision": COMBAT_STAT_COMPILER_POLICY_SEMANTIC_REVISION_V1,
        "catalog_context_policy_semantic_revision": CATALOG_CONTEXT_POLICY_SEMANTIC_REVISION_V1,
        "advisor_policy_semantic_revision": ADVISOR_POLICY_SEMANTIC_REVISION_V1,
        "battle_rule_id": BATTLE_RULE_ID,
        "fillers": sources.fillers.iter().map(|filler| key_json(filler.key)).collect::<Vec<_>>(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum WirePlayer {
    P1,
    P2,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSolve {
    #[serde(default, rename = "id")]
    _id: Value,
    #[serde(rename = "kind")]
    _kind: String,
    p1: [(u32, u8); HAND_SIZE],
    p2: [(u32, u8); HAND_SIZE],
    first: WirePlayer,
    #[serde(default)]
    night: bool,
    #[serde(default = "default_life")]
    life: u16,
    #[serde(default = "default_pillz")]
    pillz: u16,
    #[serde(default)]
    budget_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireProbe {
    #[serde(default, rename = "id")]
    _id: Value,
    #[serde(rename = "kind")]
    _kind: String,
    card: (u32, u8),
    #[serde(default)]
    night: bool,
}

const fn default_life() -> u16 {
    MATCHUP_LIFE
}

const fn default_pillz() -> u16 {
    MATCHUP_PILLZ
}

fn hand(keys: [(u32, u8); HAND_SIZE]) -> [CardKey; HAND_SIZE] {
    keys.map(|(id, level)| CardKey::new(id, level))
}

fn error_response(id: Value, kind: Option<&str>, message: String) -> Value {
    let mut object = Map::new();
    object.insert("id".to_owned(), id);
    if let Some(kind) = kind {
        object.insert("kind".to_owned(), json!(kind));
    }
    object.insert("error".to_owned(), json!(message));
    Value::Object(object)
}

/// Answers one request line. Never panics on bad input: a malformed request gets an `error`
/// response carrying whatever `id` could be read.
pub fn respond(sources: &MatchupSources, line: &str) -> Value {
    if line.len() > MAX_LINE_BYTES {
        return error_response(
            Value::Null,
            None,
            format!("request exceeds {MAX_LINE_BYTES} bytes"),
        );
    }
    let parsed: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(error) => return error_response(Value::Null, None, format!("invalid JSON: {error}")),
    };
    let id = parsed.get("id").cloned().unwrap_or(Value::Null);
    match parsed.get("kind").and_then(Value::as_str) {
        Some("solve") => match serde_json::from_value::<WireSolve>(parsed) {
            Ok(request) => respond_solve(sources, id, request),
            Err(error) => error_response(id, Some("solve"), format!("invalid solve: {error}")),
        },
        Some("probe") => match serde_json::from_value::<WireProbe>(parsed) {
            Ok(request) => {
                let card = CardKey::new(request.card.0, request.card.1);
                let result = probe(sources, card, request.night);
                json!({
                    "id": id,
                    "kind": "probe",
                    "card": key_json(card),
                    "night": request.night,
                    "status": result.status.as_str(),
                    "ability": check_json(&result.ability),
                    "bonus": check_json(&result.bonus),
                    "ability_text": result.ability_text,
                    "bonus_text": result.bonus_text,
                })
            }
            Err(error) => error_response(id, Some("probe"), format!("invalid probe: {error}")),
        },
        Some("provenance") => {
            let mut response = provenance_json(sources);
            response["id"] = id;
            response["kind"] = json!("provenance");
            response
        }
        _ => error_response(
            id,
            None,
            "kind must be \"solve\", \"probe\" or \"provenance\"".to_owned(),
        ),
    }
}

fn respond_solve(sources: &MatchupSources, id: Value, request: WireSolve) -> Value {
    if request.life == 0 || request.life > MAX_LIFE {
        return error_response(id, Some("solve"), format!("life must be 1..={MAX_LIFE}"));
    }
    if request.pillz > MAX_PILLZ {
        return error_response(id, Some("solve"), format!("pillz must be 0..={MAX_PILLZ}"));
    }
    let request = SolveRequest {
        p1: hand(request.p1),
        p2: hand(request.p2),
        first_mover: match request.first {
            WirePlayer::P1 => PlayerId::P1,
            WirePlayer::P2 => PlayerId::P2,
        },
        night: request.night,
        life: request.life,
        pillz: request.pillz,
        budget: request.budget_ms.map(Duration::from_millis),
    };
    match solve(sources, &request) {
        SolveOutcome::Solved(result) => json!({
            "id": id,
            "kind": "solve",
            "value": result.value,
            "worst": result.worst,
            "best": result.best,
            "best_move": move_json(result.best_move),
            "ko_share": result.ko_share,
            "koed_share": result.koed_share,
            "root_moves": result.root_moves,
            "replies": result.replies,
            "ms": result.elapsed.as_millis() as u64,
        }),
        SolveOutcome::Refused(reason) => json!({
            "id": id,
            "kind": "solve",
            "refused": reason,
        }),
        SolveOutcome::Incomplete { elapsed } => error_response(
            id,
            Some("solve"),
            format!(
                "budget reached after {} ms before the search completed",
                elapsed.as_millis()
            ),
        ),
    }
}

/// Reads requests until end of input, answers them on `threads` workers and writes one
/// response line per request, in request order, flushing after each.
pub fn run(
    input: impl BufRead,
    output: impl Write + Send,
    sources: Arc<MatchupSources>,
    threads: NonZeroUsize,
) -> io::Result<()> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads.get())
        .thread_name(|index| format!("matchup-{index}"))
        .build()
        .map_err(io::Error::other)?;
    let (sender, receiver) = mpsc::channel::<(usize, Value)>();
    thread::scope(|scope| {
        let writer = scope.spawn(move || -> io::Result<()> {
            let mut output = output;
            let mut next = 0;
            let mut pending = BTreeMap::new();
            for (index, response) in receiver {
                pending.insert(index, response);
                while let Some(response) = pending.remove(&next) {
                    serde_json::to_writer(&mut output, &response)?;
                    output.write_all(b"\n")?;
                    output.flush()?;
                    next += 1;
                }
            }
            Ok(())
        });
        let mut index = 0;
        let mut read_error = None;
        for line in input.lines() {
            let line = match line {
                Ok(line) => line,
                Err(error) => {
                    read_error = Some(error);
                    break;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            let sender = sender.clone();
            let sources = Arc::clone(&sources);
            let this = index;
            pool.spawn(move || {
                let response = panic::catch_unwind(AssertUnwindSafe(|| respond(&sources, &line)))
                    .unwrap_or_else(|_| {
                        error_response(Value::Null, None, "internal error".to_owned())
                    });
                // Only a failed writer drops the receiver, and then nothing more is wanted.
                let _ = sender.send((this, response));
            });
            index += 1;
        }
        drop(sender);
        let written = writer
            .join()
            .unwrap_or_else(|_| Err(io::Error::other("writer panicked")));
        match read_error {
            Some(error) => Err(error),
            None => written,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advisor::input::DEFAULT_LIFE;
    use crate::advisor::search::default_search_threads;

    fn sources() -> Arc<MatchupSources> {
        Arc::new(MatchupSources::load().expect("repository sources load"))
    }

    fn run_lines(sources: &Arc<MatchupSources>, lines: &[String], threads: usize) -> Vec<Value> {
        let input = lines.join("\n");
        let mut output = Vec::new();
        run(
            input.as_bytes(),
            &mut output,
            Arc::clone(sources),
            NonZeroUsize::new(threads).unwrap(),
        )
        .unwrap();
        String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn a_solve_is_the_advisors_exact_opening_first_decision_on_the_demo_draw() {
        let sources = sources();
        let demo = AdvisorOptions::default();
        for first_mover in PlayerId::ALL {
            let options = AdvisorOptions {
                us: first_mover,
                first_mover,
                ..AdvisorOptions::default()
            };
            let prepared =
                prepare_with_sources(options, sources.catalog(), sources.registry()).unwrap();
            let mut game = prepared.new_game();
            let advisor = search_with_threads(
                &mut game,
                SearchConfig {
                    us: first_mover,
                    first_mover,
                    mode: SearchMode::First,
                    budget: Duration::from_secs(600),
                },
                default_search_threads(),
                |_| {},
            );
            assert!(advisor.complete);
            let top = &advisor.ranked[0];

            let request = SolveRequest {
                life: DEFAULT_LIFE,
                ..SolveRequest::new(demo.p1, demo.p2, first_mover)
            };
            let SolveOutcome::Solved(result) = solve(&sources, &request) else {
                panic!("the demo draw is supported");
            };
            assert_eq!(result.best_move, top.move_, "{first_mover:?}");
            assert_eq!(
                result.value.to_bits(),
                top.average.to_bits(),
                "{first_mover:?}"
            );
            assert_eq!(
                result.worst.to_bits(),
                top.worst.to_bits(),
                "{first_mover:?}"
            );
            assert_eq!(result.root_moves, advisor.ranked.len());
            assert!((-1.0..=1.0).contains(&result.value));
        }
    }

    #[test]
    fn a_refused_draw_is_reported_as_refused_with_the_catalogs_reason() {
        let sources = sources();
        let demo = AdvisorOptions::default();
        // Ambre (269) is a Leader: the strict catalog refuses every hand holding one.
        let mut with_leader = demo.p1;
        with_leader[0] = CardKey::new(269, 5);
        let outcome = solve(
            &sources,
            &SolveRequest::new(with_leader, demo.p2, PlayerId::P1),
        );
        let SolveOutcome::Refused(reason) = outcome else {
            panic!("a Leader draw must be refused, got {outcome:?}");
        };
        assert!(reason.contains("Leader"), "{reason}");

        let line = json!({
            "id": "leader",
            "kind": "solve",
            "p1": with_leader.map(|key| (key.id, key.level)),
            "p2": demo.p2.map(|key| (key.id, key.level)),
            "first": "p2",
        })
        .to_string();
        let response = respond(&sources, &line);
        assert_eq!(response["id"], "leader");
        assert!(response["refused"].as_str().unwrap().contains("Leader"));
        assert!(response.get("value").is_none());

        // An unknown card is refused too, not guessed.
        let mut unknown = demo.p1;
        unknown[3] = CardKey::new(9_999_999, 1);
        assert!(matches!(
            solve(&sources, &SolveRequest::new(unknown, demo.p2, PlayerId::P1)),
            SolveOutcome::Refused(_)
        ));
    }

    #[test]
    fn probes_classify_a_plain_card_a_leader_and_a_missing_card() {
        let sources = sources();
        let fillers = sources.filler_keys();
        assert!(fillers.len() >= 8);

        // Natrang has no ability and a plain Damage +2 bonus.
        let plain = probe(&sources, CardKey::new(123, 1), false);
        assert_eq!(plain.status, ProbeStatus::Exact, "{plain:?}");
        assert!(matches!(
            plain.bonus,
            ProbeCheck::Passed {
                partner: Some(partner)
            } if partner.id != 123
        ));
        assert_eq!(plain.bonus_text.as_deref(), Some("Damage +2"));

        assert_eq!(
            probe(&sources, CardKey::new(269, 5), false).status,
            ProbeStatus::Leader
        );
        assert_eq!(
            probe(&sources, CardKey::new(9_999_999, 1), false).status,
            ProbeStatus::Missing
        );
    }

    #[test]
    fn responses_keep_request_order_across_threads_and_report_bad_lines() {
        let sources = sources();
        let demo = AdvisorOptions::default();
        let mut lines = vec![
            json!({"id": 0, "kind": "provenance"}).to_string(),
            "not json".to_owned(),
            String::new(),
            json!({"id": 2, "kind": "solve", "p1": [[1, 1]], "p2": [], "first": "p1"}).to_string(),
            json!({"id": 3, "kind": "probe", "card": [123, 1], "extra": true}).to_string(),
        ];
        for (index, id) in [123_u32, 269, 441, 124, 9_999_999, 138]
            .into_iter()
            .enumerate()
        {
            lines.push(
                json!({"id": index + 4, "kind": "probe", "card": [id, 1], "night": index % 2 == 1})
                    .to_string(),
            );
        }
        lines.push(
            json!({
                "id": 10,
                "kind": "solve",
                "p1": [[269, 5], [124, 1], [138, 1], [139, 1]],
                "p2": demo.p2.map(|key| (key.id, key.level)),
                "first": "p1",
            })
            .to_string(),
        );
        let serial = run_lines(&sources, &lines, 1);
        let parallel = run_lines(&sources, &lines, 4);
        assert_eq!(
            serial.len(),
            lines.len() - 1,
            "the blank line gets no response"
        );
        assert_eq!(serial, parallel);
        assert_eq!(serial[0]["kind"], "provenance");
        assert_eq!(serial[0]["protocol"], MATCHUP_PROTOCOL_VERSION);
        assert!(serial[1]["error"]
            .as_str()
            .unwrap()
            .contains("invalid JSON"));
        assert_eq!(serial[2]["id"], 2);
        assert!(serial[2]["error"].is_string());
        assert_eq!(serial[3]["id"], 3);
        assert!(
            serial[3]["error"].is_string(),
            "unknown fields are rejected"
        );
        for (offset, response) in serial[4..].iter().enumerate() {
            assert_eq!(response["id"], offset + 4);
        }
        assert_eq!(serial[4]["status"], "exact");
        assert_eq!(serial[5]["status"], "leader");
        assert_eq!(serial[8]["status"], "missing");
        assert!(serial[10]["refused"].as_str().unwrap().contains("Leader"));
    }
}
