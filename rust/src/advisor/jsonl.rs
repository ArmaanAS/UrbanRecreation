//! Strict, one-shot JSONL protocol for embedding the current Rust advisor.
//!
//! V1 is intentionally only the first-mover search.  It keeps the wire boundary small,
//! validates observed card identities against the prepared catalog/registry sources, and
//! reconstructs the supplied history through the same fully-executable catalog projection
//! as the terminal advisor.  Nothing on stdout is ever a diagnostic or a terminal frame.

use std::error::Error;
use std::fmt;
use std::io::{self, Read, Write};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::advisor::input::{repository_root, BATTLE_RULE_ID};
use crate::advisor::search::{
    search, EvaluationKind, RankedMove, SearchConfig, SearchMode, SearchSnapshot,
};
use crate::catalog::{CardKey, EffectiveCardCatalog};
use crate::effect_registry::EffectRegistryV1;
use crate::engine::{
    BaseRulesRoundInput, BaseRulesSelection, ByPlayer, CatalogCombatStatMatchInputV1,
    CatalogCombatStatMatchV1, CatalogCombatStatPlayerInputV1, CatalogCombatStatProjectionV1,
    CatalogCombatStatSourceDispositionV1, MatchStatus, PlayerId, HAND_SIZE,
};

pub const PROTOCOL_VERSION: u8 = 1;
pub const MAX_REQUEST_BYTES: usize = 65_536;
const MAX_REQUEST_ID_BYTES: usize = 128;
const MAX_BUDGET_MS: u64 = 30_000;
const MAX_LIFE: u16 = 255;
/// Every legal root action is returned, so this input bound makes records predictable.
const MAX_WORKER_PILLZ: u16 = 30;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
const MAX_PROGRESS_RECORDS: usize = 32;
const MAX_RESPONSE_BYTES: usize = 65_536;
// `MAX_PROGRESS_RECORDS` progress records plus one final record are each capped by
// `MAX_RESPONSE_BYTES` (and one newline), so stdout is bounded at 2,162,721 bytes.

#[derive(Debug)]
pub enum JsonlWorkerError {
    Io(io::Error),
    Protocol(String),
    Preparation(String),
    Output(serde_json::Error),
}

impl fmt::Display for JsonlWorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O failure: {error}"),
            Self::Protocol(message) => write!(formatter, "protocol rejection: {message}"),
            Self::Preparation(message) => {
                write!(formatter, "advisor preparation rejection: {message}")
            }
            Self::Output(error) => write!(formatter, "could not encode JSONL response: {error}"),
        }
    }
}

impl Error for JsonlWorkerError {}

impl From<io::Error> for JsonlWorkerError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Runs exactly one newline-terminated request. Protocol errors are reported only to stderr
/// through the caller and produce no stdout record: an invalid request has no trustworthy id
/// to correlate, and this keeps stdout a stream of successful protocol messages exclusively.
pub fn run(
    input: impl Read,
    mut output: impl Write,
    diagnostics: &mut impl Write,
) -> Result<(), JsonlWorkerError> {
    let request = read_request(input)?;
    let request_id = request.request_id.clone();
    let mut sequence = 0_u64;
    let mut emit =
        |kind: ResponseKind, snapshot: &SearchSnapshot| -> Result<(), JsonlWorkerError> {
            let response = Response::from_snapshot(request_id.clone(), sequence, kind, snapshot);
            sequence = sequence.checked_add(1).ok_or_else(|| {
                JsonlWorkerError::Protocol("response sequence overflow".to_owned())
            })?;
            let encoded = serde_json::to_vec(&response).map_err(JsonlWorkerError::Output)?;
            if encoded.len() > MAX_RESPONSE_BYTES {
                return Err(JsonlWorkerError::Protocol(format!(
                    "response exceeds {MAX_RESPONSE_BYTES} bytes"
                )));
            }
            output.write_all(&encoded)?;
            output.write_all(b"\n")?;
            output.flush()?;
            Ok(())
        };

    let mut game = prepare_game(&request)?;
    let config = SearchConfig {
        us: request.us.engine(),
        first_mover: request.first_mover.engine(),
        mode: SearchMode::First,
        budget: Duration::from_millis(request.budget_ms),
    };
    let mut write_failed = None;
    let mut last_progress = Duration::ZERO;
    let mut progress_records = 0;
    let final_snapshot = search(&mut game, config, |snapshot| {
        if write_failed.is_none()
            && !snapshot.complete
            && progress_records < MAX_PROGRESS_RECORDS
            && snapshot.elapsed.saturating_sub(last_progress) >= PROGRESS_INTERVAL
        {
            if let Err(error) = emit(ResponseKind::Progress, snapshot) {
                write_failed = Some(error);
            } else {
                last_progress = snapshot.elapsed;
                progress_records += 1;
            }
        }
    });
    if let Some(error) = write_failed {
        return Err(error);
    }
    emit(ResponseKind::Final, &final_snapshot)?;
    diagnostics.flush()?;
    Ok(())
}

fn read_request(mut input: impl Read) -> Result<Request, JsonlWorkerError> {
    let mut bytes = Vec::with_capacity(MAX_REQUEST_BYTES.min(4096));
    input
        .by_ref()
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(JsonlWorkerError::Protocol(format!(
            "request exceeds {MAX_REQUEST_BYTES} bytes"
        )));
    }
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err(JsonlWorkerError::Protocol(
            "expected one newline-terminated JSON object".to_owned(),
        ));
    }
    let line = &bytes[..bytes.len() - 1];
    if line.contains(&b'\n') {
        return Err(JsonlWorkerError::Protocol(
            "expected exactly one JSONL request line".to_owned(),
        ));
    }
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    let request: Request = serde_json::from_slice(line)
        .map_err(|error| JsonlWorkerError::Protocol(format!("invalid request JSON: {error}")))?;
    request.validate()?;
    Ok(request)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    protocol_version: u8,
    request_id: String,
    /// V1 accepts exactly this literal; retaining it on the wire makes expansion explicit.
    mode: String,
    /// V1 admits only requester-first searches. Named sides avoid an ambiguous boolean
    /// integration contract while allowing either P1 or P2 to be the requester.
    us: WirePlayer,
    first_mover: WirePlayer,
    battle_rule_id: u32,
    night: bool,
    provenance: Provenance,
    players: WirePlayers,
    history: Vec<HistoryRound>,
    budget_ms: u64,
}

impl Request {
    fn validate(&self) -> Result<(), JsonlWorkerError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(protocol(format!(
                "unsupported protocol_version {}; expected {PROTOCOL_VERSION}",
                self.protocol_version
            )));
        }
        if self.request_id.is_empty()
            || self.request_id.len() > MAX_REQUEST_ID_BYTES
            || !self
                .request_id
                .bytes()
                .all(|byte| (0x20..=0x7e).contains(&byte))
        {
            return Err(protocol("request_id must be 1..=128 printable ASCII bytes"));
        }
        if self.mode != "first" {
            return Err(protocol("V1 supports mode \"first\" only"));
        }
        if self.battle_rule_id != BATTLE_RULE_ID {
            return Err(protocol(&format!(
                "unsupported battle_rule_id {}; V1 supports {BATTLE_RULE_ID}",
                self.battle_rule_id
            )));
        }
        if !is_canonical_fnv1a64(&self.provenance.effective_catalog_fingerprint_fnv1a64)
            || !is_canonical_fnv1a64(&self.provenance.effect_registry_fingerprint_fnv1a64)
        {
            return Err(protocol(
                "provenance fingerprints must be exactly 16 lowercase hexadecimal characters",
            ));
        }
        if self.us != self.first_mover {
            return Err(protocol(
                "V1 first mode requires us and first_mover to name the same player",
            ));
        }
        if self.budget_ms == 0 || self.budget_ms > MAX_BUDGET_MS {
            return Err(protocol(&format!(
                "budget_ms must be in 1..={MAX_BUDGET_MS}"
            )));
        }
        if self.history.len() >= HAND_SIZE {
            return Err(protocol(
                "history must contain at most three resolved rounds",
            ));
        }
        self.players.validate()?;
        for round in &self.history {
            round.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum WirePlayer {
    P1,
    P2,
}

impl WirePlayer {
    const fn engine(self) -> PlayerId {
        match self {
            Self::P1 => PlayerId::P1,
            Self::P2 => PlayerId::P2,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    /// Exact lowercase hexadecimal FNV-1a-64 values. JSON numbers are forbidden because
    /// JavaScript cannot represent every u64 losslessly.
    effective_catalog_fingerprint_fnv1a64: String,
    effect_registry_fingerprint_fnv1a64: String,
    effect_registry_schema_version: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePlayers {
    p1: WirePlayerState,
    p2: WirePlayerState,
}

impl WirePlayers {
    fn by_player(&self, player: PlayerId) -> &WirePlayerState {
        match player {
            PlayerId::P1 => &self.p1,
            PlayerId::P2 => &self.p2,
        }
    }

    fn validate(&self) -> Result<(), JsonlWorkerError> {
        self.p1.validate("p1")?;
        self.p2.validate("p2")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePlayerState {
    initial: Resources,
    current: Resources,
    /// Redundant with history by design: it makes the host's rendered current hand state
    /// independently checkable instead of allowing the worker to silently resynchronise it.
    played: [bool; HAND_SIZE],
    hand: [ObservedCard; HAND_SIZE],
}

impl WirePlayerState {
    fn validate(&self, name: &str) -> Result<(), JsonlWorkerError> {
        self.initial.validate(&format!("players.{name}.initial"))?;
        self.current.validate(&format!("players.{name}.current"))?;
        for card in &self.hand {
            card.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resources {
    life: u16,
    pillz: u16,
}

impl Resources {
    fn validate(self, field: &str) -> Result<(), JsonlWorkerError> {
        if self.life == 0 || self.life > MAX_LIFE {
            return Err(protocol(&format!("{field}.life must be in 1..={MAX_LIFE}")));
        }
        if self.pillz > MAX_WORKER_PILLZ {
            return Err(protocol(&format!(
                "{field}.pillz must be at most {MAX_WORKER_PILLZ}"
            )));
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservedCard {
    id: u32,
    level: u8,
    ability_id: u32,
    ability: String,
    bonus_id: u32,
    bonus: String,
}

impl ObservedCard {
    fn validate(&self) -> Result<(), JsonlWorkerError> {
        if self.id == 0 || self.level == 0 || self.ability.len() > 256 || self.bonus.len() > 256 {
            return Err(protocol(
                "card id/level and observed ability/bonus strings are out of bounds",
            ));
        }
        Ok(())
    }

    const fn key(&self) -> CardKey {
        CardKey::new(self.id, self.level)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryRound {
    first_mover: WirePlayer,
    p1: WireMove,
    p2: WireMove,
}

impl HistoryRound {
    fn validate(&self) -> Result<(), JsonlWorkerError> {
        self.p1.validate()?;
        self.p2.validate()
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireMove {
    hand_index: u8,
    pillz: u16,
    fury: bool,
}

impl WireMove {
    fn validate(self) -> Result<(), JsonlWorkerError> {
        if usize::from(self.hand_index) >= HAND_SIZE {
            return Err(protocol("history hand_index must be in 0..=3"));
        }
        if self.pillz > MAX_WORKER_PILLZ {
            return Err(protocol(&format!(
                "history pillz must be at most {MAX_WORKER_PILLZ}"
            )));
        }
        Ok(())
    }

    const fn selection(self) -> BaseRulesSelection {
        BaseRulesSelection::new(self.hand_index, self.pillz, self.fury)
    }
}

fn prepare_game(
    request: &Request,
) -> Result<crate::engine::CombatStatDiagnosticV1, JsonlWorkerError> {
    let root = repository_root();
    let catalog = EffectiveCardCatalog::load(
        root.join("data/data.json"),
        root.join("data/battle_card_overrides.json"),
    )
    .map_err(|error| JsonlWorkerError::Preparation(error.to_string()))?;
    let registry = EffectRegistryV1::load(root.join("captures/abilities.json"))
        .map_err(|error| JsonlWorkerError::Preparation(error.to_string()))?;
    validate_provenance(request, &catalog, &registry)?;

    let input = CatalogCombatStatMatchInputV1 {
        battle_rule_id: request.battle_rule_id,
        night: request.night,
        players: ByPlayer::new(
            player_input(&request.players.p1),
            player_input(&request.players.p2),
        ),
    };
    let prepared = CatalogCombatStatMatchV1::new(
        input,
        &catalog,
        &registry,
        CatalogCombatStatProjectionV1::RequireFullyExecutableDraws,
    )
    .map_err(|error| JsonlWorkerError::Preparation(error.to_string()))?;
    validate_observed_cards(request, &prepared)?;
    let mut game = prepared.new_game();
    let mut expected_first = if request.history.is_empty() {
        request.first_mover.engine()
    } else {
        request.history[0].first_mover.engine()
    };
    for (round_index, round) in request.history.iter().enumerate() {
        if round.first_mover.engine() != expected_first {
            return Err(protocol(&format!(
                "history round {} first_mover does not alternate",
                round_index + 1
            )));
        }
        game.make(BaseRulesRoundInput {
            first_mover: expected_first,
            selections: ByPlayer::new(round.p1.selection(), round.p2.selection()),
        })
        .map_err(|error| {
            JsonlWorkerError::Protocol(format!(
                "illegal history round {}: {error}",
                round_index + 1
            ))
        })?;
        if game.position().status != MatchStatus::Playing
            && round_index + 1 != request.history.len()
        {
            return Err(protocol("history continues after the match ended"));
        }
        expected_first = expected_first.other();
    }
    if expected_first != request.first_mover.engine() {
        return Err(protocol(
            "first_mover must follow the explicit history alternation",
        ));
    }
    if game.position().status != MatchStatus::Playing {
        return Err(protocol("cannot advise a completed match"));
    }
    for player in PlayerId::ALL {
        let expected = request.players.by_player(player).current;
        let actual = game.position().players[player];
        if actual.life != expected.life || actual.pillz != expected.pillz {
            return Err(protocol(&format!(
                "players.{}.current does not match the strictly replayed history",
                player_name(player)
            )));
        }
        if game.position().played[player] != request.players.by_player(player).played {
            return Err(protocol(&format!(
                "players.{}.played does not match the strictly replayed history",
                player_name(player)
            )));
        }
    }
    Ok(game)
}

fn player_input(player: &WirePlayerState) -> CatalogCombatStatPlayerInputV1 {
    CatalogCombatStatPlayerInputV1 {
        initial_life: player.initial.life,
        initial_pillz: player.initial.pillz,
        hand: player.hand.each_ref().map(|card| card.key()),
    }
}

fn validate_provenance(
    request: &Request,
    catalog: &EffectiveCardCatalog,
    registry: &EffectRegistryV1,
) -> Result<(), JsonlWorkerError> {
    if request.provenance.effective_catalog_fingerprint_fnv1a64
        != fnv1a64(catalog.source_fingerprint_fnv1a64().value())
        || request.provenance.effect_registry_fingerprint_fnv1a64
            != fnv1a64(registry.source_fingerprint_fnv1a64().value())
        || request.provenance.effect_registry_schema_version != registry.schema_version()
    {
        return Err(protocol(
            "request provenance does not match this worker's strict inputs",
        ));
    }
    Ok(())
}

fn fnv1a64(value: u64) -> String {
    format!("{value:016x}")
}

fn validate_observed_cards(
    request: &Request,
    combat_match: &CatalogCombatStatMatchV1,
) -> Result<(), JsonlWorkerError> {
    for player in PlayerId::ALL {
        for (slot, observed) in request.players.by_player(player).hand.iter().enumerate() {
            let prepared = &combat_match.preparation()[player][slot];
            if prepared.key != observed.key() {
                return Err(protocol(&format!(
                    "players.{}.hand[{slot}] does not match the prepared catalog card",
                    player_name(player)
                )));
            }
            validate_observed_source(
                player,
                slot,
                "ability",
                observed.ability_id,
                &observed.ability,
                "No Ability",
                &prepared.ability,
                true,
            )?;
            validate_observed_source(
                player,
                slot,
                "bonus",
                observed.bonus_id,
                &observed.bonus,
                "No Bonus",
                &prepared.bonus,
                false,
            )?;
        }
    }
    Ok(())
}

/// The live wire cannot represent an absent source as `None`: TypeScript normalisation
/// writes the exact `0`/`No Ability` or `0`/`No Bonus` sentinel instead.  Once a source is
/// present, its text is still exact evidence, while the ID follows the same authority split
/// as captured replay grading: printed abilities retain catalog identity and clan bonuses
/// retain the registry definition selected by preparation.  Registry aliases are provenance,
/// not interchangeable execution identities.
#[allow(clippy::too_many_arguments)]
fn validate_observed_source(
    player: PlayerId,
    slot: usize,
    source_kind: &'static str,
    observed_id: u32,
    observed_description: &str,
    absent_description: &'static str,
    prepared: &CatalogCombatStatSourceDispositionV1,
    require_catalog_identity: bool,
) -> Result<(), JsonlWorkerError> {
    let mismatch = || {
        protocol(&format!(
            "players.{}.hand[{slot}] observed {source_kind} identity differs from the prepared catalog source",
            player_name(player)
        ))
    };
    let identity = match prepared {
        CatalogCombatStatSourceDispositionV1::Absent => {
            if observed_id == 0 && observed_description == absent_description {
                return Ok(());
            }
            return Err(mismatch());
        }
        CatalogCombatStatSourceDispositionV1::Execute { identity, .. }
        | CatalogCombatStatSourceDispositionV1::ExecutePostRound { identity, .. } => identity,
    };
    if observed_description != identity.description {
        return Err(mismatch());
    }
    let identity_matches = if require_catalog_identity {
        identity.catalog_id == Some(observed_id)
    } else {
        observed_id == identity.registry_definition_id
    };
    identity_matches.then_some(()).ok_or_else(mismatch)
}

const fn player_name(player: PlayerId) -> &'static str {
    match player {
        PlayerId::P1 => "p1",
        PlayerId::P2 => "p2",
    }
}

fn protocol(message: impl Into<String>) -> JsonlWorkerError {
    JsonlWorkerError::Protocol(message.into())
}

fn is_canonical_fnv1a64(value: &str) -> bool {
    value.len() == 16
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum ResponseKind {
    Progress,
    Final,
}

#[derive(Serialize)]
struct Response {
    protocol_version: u8,
    request_id: String,
    sequence: u64,
    kind: ResponseKind,
    /// Scores, extrema, and KO/loss shares are all measured in the requester's frame.
    score_frame: &'static str,
    evaluation_kind: &'static str,
    complete: bool,
    units_done: usize,
    units_total: usize,
    elapsed_ms: u128,
    ranked_moves: Vec<ResponseMove>,
}

impl Response {
    fn from_snapshot(
        request_id: String,
        sequence: u64,
        kind: ResponseKind,
        snapshot: &SearchSnapshot,
    ) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            sequence,
            kind,
            score_frame: "requester",
            evaluation_kind: match snapshot.evaluation {
                EvaluationKind::OpeningEstimate => "opening_estimate",
                EvaluationKind::ExactContinuationPolicy => "exact_continuation_policy",
            },
            complete: snapshot.complete,
            units_done: snapshot.units_done,
            units_total: snapshot.units_total,
            elapsed_ms: snapshot.elapsed.as_millis(),
            // Search publishes rows transactionally, but a fast stop leaves untouched legal
            // moves as NaN internally. Wire records carry only sampled, finite rows; a
            // complete final necessarily contains the exact full legal action set.
            ranked_moves: snapshot
                .ranked
                .iter()
                .filter(|row| row.samples != 0)
                .map(ResponseMove::from)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct ResponseMove {
    hand_index: u8,
    pillz: u16,
    fury: bool,
    score: f64,
    worst: f64,
    best: f64,
    samples: usize,
    ko_share: f64,
    loss_share: f64,
}

impl From<&RankedMove> for ResponseMove {
    fn from(row: &RankedMove) -> Self {
        debug_assert!(row.samples != 0);
        debug_assert!(row.average.is_finite() && row.worst.is_finite() && row.best.is_finite());
        let denominator = row.samples as f64;
        Self {
            hand_index: row.move_.hand_index,
            pillz: row.move_.pillz,
            fury: row.move_.fury,
            score: row.average,
            worst: row.worst,
            best: row.best,
            samples: row.samples,
            ko_share: row.kos as f64 / denominator,
            loss_share: row.koed as f64 / denominator,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{run, MAX_REQUEST_BYTES};
    use crate::advisor::input::repository_root;
    use crate::catalog::{CardKey, EffectiveCardCatalog};
    use crate::effect_registry::EffectRegistryV1;
    use crate::engine::{
        BaseRulesRoundInput, BaseRulesSelection, ByPlayer, CatalogCombatStatMatchInputV1,
        CatalogCombatStatMatchV1, CatalogCombatStatPlayerInputV1, CatalogCombatStatProjectionV1,
        CatalogCombatStatSourceDispositionV1, PlayerId, HAND_SIZE,
    };
    use serde_json::{json, Value};

    fn observed_source(
        source: &CatalogCombatStatSourceDispositionV1,
        absent_description: &str,
        require_catalog_identity: bool,
    ) -> (u32, String) {
        let identity = match source {
            CatalogCombatStatSourceDispositionV1::Absent => {
                return (0, absent_description.to_owned());
            }
            CatalogCombatStatSourceDispositionV1::Execute { identity, .. }
            | CatalogCombatStatSourceDispositionV1::ExecutePostRound { identity, .. } => identity,
        };
        let id = if require_catalog_identity {
            identity
                .catalog_id
                .expect("the daytime fixture has printed ability identities")
        } else {
            identity.registry_definition_id
        };
        (id, identity.description.clone())
    }

    fn prepared_wire_card(
        catalog: &EffectiveCardCatalog,
        prepared: &CatalogCombatStatMatchV1,
        player: PlayerId,
        slot: usize,
    ) -> Value {
        let source = &prepared.preparation()[player][slot];
        let card = catalog.get(source.key).unwrap();
        let (ability_id, ability) = observed_source(&source.ability, "No Ability", true);
        let (bonus_id, bonus) = observed_source(&source.bonus, "No Bonus", false);
        json!({
            "id": card.id, "level": card.level,
            "ability_id": ability_id, "ability": ability,
            "bonus_id": bonus_id, "bonus": bonus,
        })
    }

    fn valid_request() -> Value {
        let root = repository_root();
        let catalog = EffectiveCardCatalog::load(
            root.join("data/data.json"),
            root.join("data/battle_card_overrides.json"),
        )
        .unwrap();
        let registry = EffectRegistryV1::load(root.join("captures/abilities.json")).unwrap();
        let p1 = [
            CardKey::new(123, 1),
            CardKey::new(124, 1),
            CardKey::new(138, 1),
            CardKey::new(139, 1),
        ];
        let p2 = [
            CardKey::new(441, 1),
            CardKey::new(444, 1),
            CardKey::new(445, 1),
            CardKey::new(447, 1),
        ];
        let prepared = CatalogCombatStatMatchV1::new(
            CatalogCombatStatMatchInputV1 {
                battle_rule_id: 10,
                night: false,
                players: ByPlayer::new(
                    CatalogCombatStatPlayerInputV1 {
                        initial_life: 14,
                        initial_pillz: 0,
                        hand: p1,
                    },
                    CatalogCombatStatPlayerInputV1 {
                        initial_life: 14,
                        initial_pillz: 0,
                        hand: p2,
                    },
                ),
            },
            &catalog,
            &registry,
            CatalogCombatStatProjectionV1::RequireFullyExecutableDraws,
        )
        .unwrap();
        let p1_hand = (0..HAND_SIZE)
            .map(|slot| prepared_wire_card(&catalog, &prepared, PlayerId::P1, slot))
            .collect::<Vec<_>>();
        let p2_hand = (0..HAND_SIZE)
            .map(|slot| prepared_wire_card(&catalog, &prepared, PlayerId::P2, slot))
            .collect::<Vec<_>>();
        json!({
            "protocol_version": 1,
            "request_id": "fixture-1",
            "mode": "first",
            "us": "p1",
            "first_mover": "p1",
            "battle_rule_id": 10,
            "night": false,
            "provenance": {
                "effective_catalog_fingerprint_fnv1a64": format!("{:016x}", catalog.source_fingerprint_fnv1a64().value()),
                "effect_registry_fingerprint_fnv1a64": format!("{:016x}", registry.source_fingerprint_fnv1a64().value()),
                "effect_registry_schema_version": registry.schema_version(),
            },
            "players": {
                "p1": {"initial": {"life": 14, "pillz": 0}, "current": {"life": 14, "pillz": 0}, "played": [false, false, false, false], "hand": p1_hand},
                "p2": {"initial": {"life": 14, "pillz": 0}, "current": {"life": 14, "pillz": 0}, "played": [false, false, false, false], "hand": p2_hand},
            },
            "history": [],
            "budget_ms": 50,
        })
    }

    fn capture_1024673_opening_request() -> Value {
        let root = repository_root();
        let catalog = EffectiveCardCatalog::load(
            root.join("data/data.json"),
            root.join("data/battle_card_overrides.json"),
        )
        .unwrap();
        let registry = EffectRegistryV1::load(root.join("captures/abilities.json")).unwrap();
        json!({
            "protocol_version": 1,
            "request_id": "capture-1024673-opening",
            "mode": "first",
            "us": "p1",
            "first_mover": "p1",
            "battle_rule_id": 10,
            "night": true,
            "provenance": {
                "effective_catalog_fingerprint_fnv1a64": format!("{:016x}", catalog.source_fingerprint_fnv1a64().value()),
                "effect_registry_fingerprint_fnv1a64": format!("{:016x}", registry.source_fingerprint_fnv1a64().value()),
                "effect_registry_schema_version": registry.schema_version(),
            },
            "players": {
                "p1": {"initial": {"life": 12, "pillz": 12}, "current": {"life": 12, "pillz": 12}, "played": [false, false, false, false], "hand": [
                    {"id": 1983, "level": 5, "ability_id": 1848, "ability": "Symmetry: -3 Opp Power, Min 2", "bonus_id": 1844, "bonus": "Asymmetry: Damage +3"},
                    {"id": 1985, "level": 2, "ability_id": 1850, "ability": "Courage: -2 Opp Pow. & Dam., Min 2", "bonus_id": 1844, "bonus": "Asymmetry: Damage +3"},
                    {"id": 1986, "level": 3, "ability_id": 0, "ability": "No Ability", "bonus_id": 1844, "bonus": "Asymmetry: Damage +3"},
                    {"id": 2179, "level": 3, "ability_id": 2535, "ability": "Support: -1 Opp Attack, Min 0", "bonus_id": 1844, "bonus": "Asymmetry: Damage +3"}
                ]},
                "p2": {"initial": {"life": 12, "pillz": 12}, "current": {"life": 12, "pillz": 12}, "played": [false, false, false, false], "hand": [
                    {"id": 189, "level": 2, "ability_id": 73, "ability": "Stop Opp. Ability", "bonus_id": 6, "bonus": "-12 Opp Attack, Min 8"},
                    {"id": 413, "level": 1, "ability_id": 4299, "ability": "-2 Opp Power, Min 4", "bonus_id": 6, "bonus": "-12 Opp Attack, Min 8"},
                    {"id": 2349, "level": 3, "ability_id": 3366, "ability": "Equalizer: -1 Opp Power, Min 0", "bonus_id": 333, "bonus": "Stop Opp. Bonus"},
                    {"id": 1962, "level": 2, "ability_id": 1825, "ability": "-4 Opp Power, Min 0", "bonus_id": 333, "bonus": "Stop Opp. Bonus"}
                ]},
            },
            "history": [],
            "budget_ms": 1,
        })
    }

    fn invoke(value: Value) -> (Result<(), String>, String, String) {
        let input = format!("{}\n", serde_json::to_string(&value).unwrap());
        let mut output = Vec::new();
        let mut diagnostics = Vec::new();
        let result =
            run(input.as_bytes(), &mut output, &mut diagnostics).map_err(|error| error.to_string());
        (
            result,
            String::from_utf8(output).unwrap(),
            String::from_utf8(diagnostics).unwrap(),
        )
    }

    fn assert_rejected(value: Value) {
        let (result, stdout, _) = invoke(value);
        assert!(result.is_err());
        assert!(stdout.is_empty());
    }

    fn valid_one_round_request() -> Value {
        let mut request = valid_request();
        let parsed: super::Request = serde_json::from_value(request.clone()).unwrap();
        parsed.validate().unwrap();
        let mut game = super::prepare_game(&parsed).unwrap();
        let (report, _) = game
            .make(BaseRulesRoundInput {
                first_mover: PlayerId::P1,
                selections: ByPlayer::new(
                    BaseRulesSelection::new(0, 0, false),
                    BaseRulesSelection::new(0, 0, false),
                ),
            })
            .unwrap();
        request["us"] = json!("p2");
        request["first_mover"] = json!("p2");
        request["history"] = json!([{
            "first_mover": "p1",
            "p1": {"hand_index": 0, "pillz": 0, "fury": false},
            "p2": {"hand_index": 0, "pillz": 0, "fury": false},
        }]);
        for (wire, player) in [("p1", PlayerId::P1), ("p2", PlayerId::P2)] {
            request["players"][wire]["current"] = json!({
                "life": report.players[player].life,
                "pillz": report.players[player].pillz,
            });
            request["players"][wire]["played"] = json!([true, false, false, false]);
        }
        request
    }

    #[test]
    fn deterministic_fixture_emits_only_versioned_jsonl_progress_and_final() {
        let (result, stdout, stderr) = invoke(valid_request());
        result.unwrap();
        assert!(stderr.is_empty());
        let lines: Vec<Value> = stdout
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(!lines.is_empty(), "expected a final record: {stdout}");
        for (index, line) in lines.iter().enumerate() {
            assert_eq!(line["protocol_version"], 1);
            assert_eq!(line["request_id"], "fixture-1");
            assert_eq!(line["sequence"], index as u64);
            assert_eq!(line["evaluation_kind"], "opening_estimate");
            assert_eq!(line["score_frame"], "requester");
            assert!(line["ranked_moves"].is_array());
            for move_ in line["ranked_moves"].as_array().unwrap() {
                assert!(move_["score"].is_number());
                assert!(move_["worst"].is_number());
                assert!(move_["best"].is_number());
                assert!(move_["ko_share"].is_number());
                assert!(move_["loss_share"].is_number());
            }
        }
        assert_eq!(lines.last().unwrap()["kind"], "final");
    }

    #[test]
    fn capture_1024673_accepts_registry_bonus_identity_and_rejects_catalog_id() {
        let request = capture_1024673_opening_request();
        assert_eq!(request["players"]["p1"]["hand"][0]["bonus_id"], 1844);
        let (result, stdout, stderr) = invoke(request.clone());
        result.unwrap();
        assert!(stderr.is_empty());
        assert_eq!(
            serde_json::from_str::<Value>(stdout.lines().last().unwrap()).unwrap()["kind"],
            "final"
        );

        // The catalog's printed Asymmetry clan-bonus id is 54, but the capture supplies
        // the selected registry definition 1844.  Same text is not enough to make the
        // catalog id an acceptable capture identity.
        let mut malformed = request;
        malformed["players"]["p1"]["hand"][0]["bonus_id"] = json!(54);
        let (result, stdout, _) = invoke(malformed);
        let error = result.unwrap_err();
        assert!(error.contains("players.p1.hand[0] observed bonus identity"));
        assert!(stdout.is_empty());
    }

    #[test]
    fn first_mode_accepts_either_requesting_side_when_it_moves_first() {
        let mut request = valid_request();
        request["us"] = json!("p2");
        request["first_mover"] = json!("p2");
        let (result, stdout, _) = invoke(request);
        result.unwrap();
        let final_record: Value = serde_json::from_str(stdout.lines().last().unwrap()).unwrap();
        assert_eq!(final_record["kind"], "final");
        assert_eq!(final_record["score_frame"], "requester");
    }

    #[test]
    fn one_round_history_reconstructs_masks_resources_and_p2_first_orientation() {
        let (result, stdout, _) = invoke(valid_one_round_request());
        result.unwrap();
        let final_record: Value = serde_json::from_str(stdout.lines().last().unwrap()).unwrap();
        assert_eq!(final_record["kind"], "final");
        assert_eq!(final_record["score_frame"], "requester");
        assert_eq!(final_record["evaluation_kind"], "exact_continuation_policy");
    }

    #[test]
    fn rejects_malformed_oversize_version_mode_and_card_identity_without_stdout() {
        let mut output = Vec::new();
        let mut diagnostics = Vec::new();
        assert!(run(b"not json\n".as_slice(), &mut output, &mut diagnostics).is_err());
        assert!(output.is_empty());

        let oversized = vec![b'x'; MAX_REQUEST_BYTES + 1];
        assert!(run(oversized.as_slice(), &mut output, &mut diagnostics).is_err());
        assert!(output.is_empty());

        for (field, value) in [
            ("protocol_version", json!(2)),
            ("mode", json!("second")),
            ("battle_rule_id", json!(11)),
        ] {
            let mut request = valid_request();
            request[field] = value;
            let (result, stdout, _) = invoke(request);
            assert!(result.is_err(), "{field} should reject");
            assert!(stdout.is_empty());
        }
        let mut request = valid_request();
        request["players"]["p1"]["hand"][0]["ability"] = json!("forged");
        let (result, stdout, _) = invoke(request);
        assert!(result.is_err());
        assert!(stdout.is_empty());

        let mut request = valid_request();
        request["players"]["p1"]["played"][0] = json!(true);
        let (result, stdout, _) = invoke(request);
        assert!(result.is_err());
        assert!(stdout.is_empty());
    }

    #[test]
    fn rejects_provenance_unknown_fields_roles_and_inconsistent_history() {
        let mut request = valid_request();
        request["provenance"]["effective_catalog_fingerprint_fnv1a64"] = json!("0000000000000000");
        assert_rejected(request);

        let mut request = valid_request();
        request["provenance"]["effect_registry_fingerprint_fnv1a64"] = json!("ABCDEF0123456789");
        assert_rejected(request);

        let mut request = valid_request();
        request["unexpected"] = json!(true);
        assert_rejected(request);

        let mut request = valid_request();
        request["first_mover"] = json!("p2");
        assert_rejected(request);

        let mut request = valid_request();
        request["history"] = json!([
            {"first_mover": "p1", "p1": {"hand_index": 0, "pillz": 0, "fury": false}, "p2": {"hand_index": 0, "pillz": 0, "fury": false}},
            {"first_mover": "p1", "p1": {"hand_index": 1, "pillz": 0, "fury": false}, "p2": {"hand_index": 1, "pillz": 0, "fury": false}}
        ]);
        assert_rejected(request);

        let mut request = valid_request();
        request["history"] = json!([{
            "first_mover": "p1",
            "p1": {"hand_index": 0, "pillz": 1, "fury": false},
            "p2": {"hand_index": 0, "pillz": 0, "fury": false}
        }]);
        assert_rejected(request);
    }

    #[test]
    fn rejects_current_resource_mismatch_after_valid_history_replay() {
        let mut request = valid_one_round_request();
        request["players"]["p1"]["current"]["life"] = json!(1);
        assert_rejected(request);
    }

    #[test]
    fn rejects_excess_jsonl_input() {
        let request = serde_json::to_string(&valid_request()).unwrap();
        let input = format!("{request}\n{request}\n");
        let mut output = Vec::new();
        let mut diagnostics = Vec::new();
        assert!(run(input.as_bytes(), &mut output, &mut diagnostics).is_err());
        assert!(output.is_empty());
    }
}
