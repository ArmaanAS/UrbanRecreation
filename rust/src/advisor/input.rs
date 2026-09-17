//! Strict, deterministic command-line input for the projected advisor.
//!
//! This module deliberately owns neither search policy nor terminal rendering.  It turns a
//! small explicit CLI surface into the catalog-backed match that those layers consume, and
//! refuses a draw whenever the catalog projection cannot execute every selectable card.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use crate::catalog::{CardKey, EffectiveCardCatalog, EffectiveCatalogError};
use crate::effect_registry::{EffectRegistryError, EffectRegistryV1};
use crate::engine::{
    ByPlayer, CatalogCombatStatMatchErrorV1, CatalogCombatStatMatchInputV1,
    CatalogCombatStatMatchV1, CatalogCombatStatPlayerInputV1, CatalogCombatStatProjectionV1,
    CatalogCombatStatSourceDispositionV1, PlayerId, HAND_SIZE,
};
use crate::replay::{
    adapt_capture, AdapterError, CapturedGame, EnginePlayer, ReplayCaseV1, ReplayClassification,
    ReplayRound, SourceModifier,
};

pub const DEFAULT_LIFE: u16 = 14;
pub const DEFAULT_PILLZ: u16 = 12;
pub const DEFAULT_BUDGET_MS: u64 = 1_000;
pub const DEFAULT_WIDTH: u16 = 80;
pub const DEFAULT_HEIGHT: u16 = 24;
/// A generous guardrail for a manual current position. Normal games begin with 12 pillz.
pub const MAX_ADVISOR_PILLZ: u16 = 255;
pub const BATTLE_RULE_ID: u32 = 10;

const DEMO_P1: [CardKey; HAND_SIZE] = [
    CardKey::new(123, 1),
    CardKey::new(124, 1),
    CardKey::new(138, 1),
    CardKey::new(139, 1),
];
const DEMO_P2: [CardKey; HAND_SIZE] = [
    CardKey::new(441, 1),
    CardKey::new(444, 1),
    CardKey::new(445, 1),
    CardKey::new(447, 1),
];

pub const USAGE: &str = "Usage: advisor [--demo | --p1 id:level,... --p2 id:level,... | --replay BATTLE_ID] [--life N] [--pillz N] [--night] [--us p1|p2] [--first p1|p2] [--second-card 0..3 | --interactive] [--budget-ms N] [--width N] [--height N] [--plain]\n\nWith no arguments, advisor uses the deterministic supported demo draw. --interactive advances a complete manual match. --replay grades every recorded decision through the strict Rust engine and solver; it may be combined only with budget/display flags. One-shot second-mover advice requires --second-card.";

/// All non-card controls are explicit, while the two hands remain fixed-size card keys.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdvisorOptions {
    pub p1: [CardKey; HAND_SIZE],
    pub p2: [CardKey; HAND_SIZE],
    pub life: u16,
    pub pillz: u16,
    pub night: bool,
    pub us: PlayerId,
    pub first_mover: PlayerId,
    /// The opponent's revealed card when we move second, indexed in that player's hand.
    pub second_card: Option<u8>,
    pub budget_ms: u64,
    pub width: u16,
    pub height: u16,
    pub plain: bool,
    pub interactive: bool,
    pub using_demo_draw: bool,
    /// A normalized capture to replay decision by decision. Its hands, resources, roles,
    /// night state, and first movers are authoritative over the manual defaults above.
    pub replay_id: Option<u64>,
}

impl Default for AdvisorOptions {
    fn default() -> Self {
        Self {
            p1: DEMO_P1,
            p2: DEMO_P2,
            life: DEFAULT_LIFE,
            pillz: DEFAULT_PILLZ,
            night: false,
            us: PlayerId::P1,
            first_mover: PlayerId::P1,
            second_card: None,
            budget_ms: DEFAULT_BUDGET_MS,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            plain: false,
            interactive: false,
            using_demo_draw: true,
            replay_id: None,
        }
    }
}

/// Immutable normalized capture data retained by replay mode after strict draw preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedAdvisorReplay {
    pub battle_id: u64,
    pub us: PlayerId,
    pub player_names: ByPlayer<String>,
    pub rounds: Vec<ReplayRound>,
}

/// Display data is copied from the same effective catalog that produced the strict match.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdvisorCardDisplay {
    pub key: CardKey,
    pub name: String,
    pub clan_name: String,
    pub power: u8,
    pub damage: u8,
}

/// The one input object passed into the search and view layers.
///
/// `combat_match` was constructed with `RequireFullyExecutableDraws`; callers must not
/// replace it with a legacy game or silently turn unsupported sources into no-ops.
#[derive(Debug)]
pub struct PreparedAdvisorInput {
    pub options: AdvisorOptions,
    pub combat_match: CatalogCombatStatMatchV1,
    pub cards: ByPlayer<[AdvisorCardDisplay; HAND_SIZE]>,
    pub source_label: String,
    pub replay: Option<PreparedAdvisorReplay>,
}

impl PreparedAdvisorInput {
    pub fn new_game(&self) -> crate::engine::CombatStatDiagnosticV1 {
        self.combat_match.new_game()
    }
}

#[derive(Debug)]
pub enum AdvisorInputError {
    Argument(String),
    Catalog(EffectiveCatalogError),
    Registry(EffectRegistryError),
    ReplayOpen {
        path: PathBuf,
        source: io::Error,
    },
    ReplayParse {
        battle_id: u64,
        source: serde_json::Error,
    },
    ReplayAdapt(AdapterError),
    ReplayUnavailable {
        battle_id: u64,
        reason: String,
    },
    ReplayRecordingSideMissing {
        battle_id: u64,
    },
    ReplayEvidenceMissing {
        battle_id: u64,
        round: u8,
        player: PlayerId,
    },
    ReplaySourceMismatch {
        battle_id: u64,
        player: PlayerId,
        hand_slot: u8,
        source_kind: &'static str,
        detail: String,
    },
    UnsupportedDraw(CatalogCombatStatMatchErrorV1),
}

impl fmt::Display for AdvisorInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Argument(message) => write!(formatter, "invalid advisor arguments: {message}"),
            Self::Catalog(source) => {
                write!(formatter, "failed to load effective catalog: {source}")
            }
            Self::Registry(source) => write!(formatter, "failed to load effect registry: {source}"),
            Self::ReplayOpen { path, source } => {
                write!(formatter, "failed to open replay {}: {source}", path.display())
            }
            Self::ReplayParse { battle_id, source } => {
                write!(formatter, "failed to parse replay battle {battle_id}: {source}")
            }
            Self::ReplayAdapt(source) => source.fmt(formatter),
            Self::ReplayUnavailable { battle_id, reason } => {
                write!(formatter, "battle {battle_id} is not replayable: {reason}")
            }
            Self::ReplayRecordingSideMissing { battle_id } => write!(
                formatter,
                "battle {battle_id} does not identify the recording player, so its decisions cannot be graded"
            ),
            Self::ReplayEvidenceMissing {
                battle_id,
                round,
                player,
            } => write!(
                formatter,
                "battle {battle_id} round {} has no server card result for {player:?}; strict replay advice requires power, damage, attack, winner, life, and pillz evidence",
                round + 1
            ),
            Self::ReplaySourceMismatch {
                battle_id,
                player,
                hand_slot,
                source_kind,
                detail,
            } => write!(
                formatter,
                "battle {battle_id} {player:?} slot {hand_slot} {source_kind} differs from the strict catalog match: {detail}"
            ),
            Self::UnsupportedDraw(source) => write!(formatter, "unsupported draw: {source}"),
        }
    }
}

impl Error for AdvisorInputError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Catalog(source) => Some(source),
            Self::Registry(source) => Some(source),
            Self::ReplayOpen { source, .. } => Some(source),
            Self::ReplayParse { source, .. } => Some(source),
            Self::ReplayAdapt(source) => Some(source),
            Self::UnsupportedDraw(source) => Some(source),
            Self::Argument(_)
            | Self::ReplayUnavailable { .. }
            | Self::ReplayRecordingSideMissing { .. }
            | Self::ReplayEvidenceMissing { .. }
            | Self::ReplaySourceMismatch { .. } => None,
        }
    }
}

pub enum AdvisorCommand {
    Help,
    Run(AdvisorOptions),
}

/// Parses command-line arguments after the executable name.  No abbreviated, positional, or
/// `--flag=value` spellings are admitted so invocation records stay unambiguous.
pub fn parse_args<I>(arguments: I) -> Result<AdvisorCommand, AdvisorInputError>
where
    I: IntoIterator<Item = String>,
{
    let arguments: Vec<String> = arguments.into_iter().collect();
    if arguments.iter().any(|argument| argument == "--help") {
        return if arguments.len() == 1 {
            Ok(AdvisorCommand::Help)
        } else {
            Err(argument_error("--help cannot be combined with other flags"))
        };
    }

    let mut options = AdvisorOptions::default();
    let mut p1 = None;
    let mut p2 = None;
    let mut explicit_demo = false;
    let mut seen = BTreeSet::new();
    let mut index = 0;
    while index < arguments.len() {
        let flag = &arguments[index];
        match flag.as_str() {
            "--demo" => {
                mark_once(&mut seen, flag)?;
                explicit_demo = true;
            }
            "--replay" => {
                mark_once(&mut seen, flag)?;
                options.replay_id = Some(parse_nonzero(
                    value_after(&arguments, &mut index, flag)?,
                    flag,
                )?);
            }
            "--p1" => {
                mark_once(&mut seen, flag)?;
                p1 = Some(parse_hand(
                    value_after(&arguments, &mut index, flag)?,
                    flag,
                )?);
            }
            "--p2" => {
                mark_once(&mut seen, flag)?;
                p2 = Some(parse_hand(
                    value_after(&arguments, &mut index, flag)?,
                    flag,
                )?);
            }
            "--life" => {
                mark_once(&mut seen, flag)?;
                options.life = parse_nonzero(value_after(&arguments, &mut index, flag)?, flag)?;
            }
            "--pillz" => {
                mark_once(&mut seen, flag)?;
                options.pillz = parse_pillz(value_after(&arguments, &mut index, flag)?)?;
            }
            "--night" => {
                mark_once(&mut seen, flag)?;
                options.night = true;
            }
            "--us" => {
                mark_once(&mut seen, flag)?;
                options.us = parse_player(value_after(&arguments, &mut index, flag)?, flag)?;
            }
            "--first" => {
                mark_once(&mut seen, flag)?;
                options.first_mover =
                    parse_player(value_after(&arguments, &mut index, flag)?, flag)?;
            }
            "--second-card" => {
                mark_once(&mut seen, flag)?;
                options.second_card = Some(parse_second_card(value_after(
                    &arguments, &mut index, flag,
                )?)?);
            }
            "--budget-ms" => {
                mark_once(&mut seen, flag)?;
                options.budget_ms =
                    parse_nonzero(value_after(&arguments, &mut index, flag)?, flag)?;
            }
            "--width" => {
                mark_once(&mut seen, flag)?;
                options.width = parse_nonzero(value_after(&arguments, &mut index, flag)?, flag)?;
            }
            "--height" => {
                mark_once(&mut seen, flag)?;
                options.height = parse_nonzero(value_after(&arguments, &mut index, flag)?, flag)?;
            }
            "--plain" => {
                mark_once(&mut seen, flag)?;
                options.plain = true;
            }
            "--interactive" => {
                mark_once(&mut seen, flag)?;
                options.interactive = true;
            }
            _ if flag.starts_with("--") => {
                return Err(argument_error(&format!("unknown flag {flag:?}")));
            }
            _ => {
                return Err(argument_error(&format!(
                    "unexpected positional argument {flag:?}"
                )))
            }
        }
        index += 1;
    }

    match (p1, p2) {
        (None, None) => {}
        (Some(p1_hand), Some(p2_hand)) => {
            if explicit_demo {
                return Err(argument_error("--demo conflicts with --p1/--p2"));
            }
            options.p1 = p1_hand;
            options.p2 = p2_hand;
            options.using_demo_draw = false;
        }
        _ => return Err(argument_error("--p1 and --p2 must be supplied together")),
    }
    if options.replay_id.is_some() {
        let conflicts = [
            "--demo",
            "--p1",
            "--p2",
            "--life",
            "--pillz",
            "--night",
            "--us",
            "--first",
            "--second-card",
            "--interactive",
        ];
        if let Some(conflict) = conflicts.iter().find(|flag| seen.contains(**flag)) {
            return Err(argument_error(&format!(
                "--replay cannot be combined with {conflict}"
            )));
        }
        options.using_demo_draw = false;
    }
    if options.interactive && options.second_card.is_some() {
        return Err(argument_error(
            "--interactive prompts for each revealed card and conflicts with --second-card",
        ));
    }
    if !options.interactive && options.second_card.is_some() && options.us == options.first_mover {
        return Err(argument_error(
            "--second-card requires --us to be the second mover",
        ));
    }
    if !options.interactive && options.second_card.is_none() && options.us != options.first_mover {
        return Err(argument_error(
            "--us is the second mover; provide the revealed opponent card with --second-card 0..3",
        ));
    }
    Ok(AdvisorCommand::Run(options))
}

/// Loads the versioned repository inputs used by the strict catalog boundary. Replay mode
/// additionally normalizes one captured game through the same adapter as the replay gate.
pub fn prepare(mut options: AdvisorOptions) -> Result<PreparedAdvisorInput, AdvisorInputError> {
    let root = repository_root();
    let catalog = EffectiveCardCatalog::load(
        root.join("data/data.json"),
        root.join("data/battle_card_overrides.json"),
    )
    .map_err(AdvisorInputError::Catalog)?;
    let registry = EffectRegistryV1::load(root.join("captures/abilities.json"))
        .map_err(AdvisorInputError::Registry)?;
    let Some(battle_id) = options.replay_id else {
        return prepare_with_sources(options, &catalog, &registry);
    };

    let replay = load_replay(&root, battle_id, &catalog)?;
    let recording_side = replay
        .metadata
        .recording_side
        .ok_or(AdvisorInputError::ReplayRecordingSideMissing { battle_id })?;
    let us = replay
        .players
        .iter()
        .find(|player| player.source_side == recording_side)
        .map(|player| engine_player(player.engine_player))
        .ok_or(AdvisorInputError::ReplayRecordingSideMissing { battle_id })?;
    let first_round =
        replay
            .rounds
            .first()
            .ok_or_else(|| AdvisorInputError::ReplayUnavailable {
                battle_id,
                reason: "the normalized capture contains no resolved rounds".to_owned(),
            })?;
    validate_replay_evidence(&replay)?;
    let hands = ByPlayer::new(
        replay.players[0].hand.clone().map(|card| card.key),
        replay.players[1].hand.clone().map(|card| card.key),
    );
    options.p1 = hands[PlayerId::P1];
    options.p2 = hands[PlayerId::P2];
    options.life = replay.players[0].base_life;
    options.pillz = replay.players[0].base_pillz;
    options.night = replay.metadata.night;
    options.us = us;
    options.first_mover = engine_player(first_round.first_mover);
    options.second_card = None;
    options.interactive = false;
    options.using_demo_draw = false;

    let player_names = ByPlayer::new(
        replay.players[0].profile.name.clone(),
        replay.players[1].profile.name.clone(),
    );
    let replay_data = PreparedAdvisorReplay {
        battle_id,
        us,
        player_names,
        rounds: replay.rounds.clone(),
    };
    let input = CatalogCombatStatMatchInputV1 {
        battle_rule_id: replay.metadata.battle_rule_id,
        night: replay.metadata.night,
        players: ByPlayer::new(
            player_input(
                hands[PlayerId::P1],
                replay.players[0].base_life,
                replay.players[0].base_pillz,
            ),
            player_input(
                hands[PlayerId::P2],
                replay.players[1].base_life,
                replay.players[1].base_pillz,
            ),
        ),
    };
    let prepared = prepare_match(
        options,
        input,
        &catalog,
        &registry,
        format!("REPLAY {battle_id}"),
        Some(replay_data),
    )?;
    validate_replay_sources(&replay, &prepared.combat_match)?;
    Ok(prepared)
}

/// Kept public for deterministic tests and for a future embedding host that owns validated
/// source bytes.  It retains the exact same fully-executable projection as [`prepare`].
pub fn prepare_with_sources(
    options: AdvisorOptions,
    catalog: &EffectiveCardCatalog,
    registry: &EffectRegistryV1,
) -> Result<PreparedAdvisorInput, AdvisorInputError> {
    if options.replay_id.is_some() {
        return Err(argument_error(
            "prepare_with_sources cannot load --replay; use prepare() with repository capture data",
        ));
    }
    let input = CatalogCombatStatMatchInputV1 {
        battle_rule_id: BATTLE_RULE_ID,
        night: options.night,
        players: ByPlayer::new(
            player_input(options.p1, options.life, options.pillz),
            player_input(options.p2, options.life, options.pillz),
        ),
    };
    let source_label = if options.using_demo_draw {
        "DEMO".to_owned()
    } else {
        "CATALOG".to_owned()
    };
    prepare_match(options, input, catalog, registry, source_label, None)
}

fn prepare_match(
    options: AdvisorOptions,
    input: CatalogCombatStatMatchInputV1,
    catalog: &EffectiveCardCatalog,
    registry: &EffectRegistryV1,
    source_label: String,
    replay: Option<PreparedAdvisorReplay>,
) -> Result<PreparedAdvisorInput, AdvisorInputError> {
    let combat_match = CatalogCombatStatMatchV1::new(
        input,
        catalog,
        registry,
        CatalogCombatStatProjectionV1::RequireFullyExecutableDraws,
    )
    .map_err(AdvisorInputError::UnsupportedDraw)?;
    let cards = ByPlayer::new(
        display_hand(options.p1, catalog),
        display_hand(options.p2, catalog),
    );
    Ok(PreparedAdvisorInput {
        options,
        combat_match,
        cards,
        source_label,
        replay,
    })
}

fn load_replay(
    root: &Path,
    battle_id: u64,
    catalog: &EffectiveCardCatalog,
) -> Result<Box<ReplayCaseV1>, AdvisorInputError> {
    let path = root.join(format!("captures/games/{battle_id}.json"));
    let file = File::open(&path).map_err(|source| AdvisorInputError::ReplayOpen {
        path: path.clone(),
        source,
    })?;
    let capture = CapturedGame::from_reader(file)
        .map_err(|source| AdvisorInputError::ReplayParse { battle_id, source })?;
    match adapt_capture(capture, catalog.as_catalog()).map_err(AdvisorInputError::ReplayAdapt)? {
        ReplayClassification::Ready(replay) => Ok(replay),
        ReplayClassification::Skipped(skipped) => Err(AdvisorInputError::ReplayUnavailable {
            battle_id,
            reason: format!(
                "{:?} (source status {})",
                skipped.reason, skipped.source_status
            ),
        }),
    }
}

fn validate_replay_sources(
    replay: &ReplayCaseV1,
    combat_match: &CatalogCombatStatMatchV1,
) -> Result<(), AdvisorInputError> {
    for player in PlayerId::ALL {
        let index = match player {
            PlayerId::P1 => 0,
            PlayerId::P2 => 1,
        };
        for hand_slot in 0..HAND_SIZE {
            let captured = &replay.players[index].hand[hand_slot];
            let prepared = &combat_match.preparation()[player][hand_slot];
            validate_replay_source(
                replay.metadata.battle_id,
                player,
                hand_slot as u8,
                "Ability",
                captured.source_ability.as_ref(),
                &prepared.ability,
                true,
            )?;
            validate_replay_source(
                replay.metadata.battle_id,
                player,
                hand_slot as u8,
                "Bonus",
                captured.source_bonus.as_ref(),
                &prepared.bonus,
                false,
            )?;
        }
    }
    Ok(())
}

fn validate_replay_evidence(replay: &ReplayCaseV1) -> Result<(), AdvisorInputError> {
    for round in &replay.rounds {
        for (index, result) in round.expected_card_results.iter().enumerate() {
            if result.is_none() {
                return Err(AdvisorInputError::ReplayEvidenceMissing {
                    battle_id: replay.metadata.battle_id,
                    round: round.round,
                    player: if index == 0 {
                        PlayerId::P1
                    } else {
                        PlayerId::P2
                    },
                });
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_replay_source(
    battle_id: u64,
    player: PlayerId,
    hand_slot: u8,
    source_kind: &'static str,
    captured: Option<&SourceModifier>,
    prepared: &CatalogCombatStatSourceDispositionV1,
    require_catalog_identity: bool,
) -> Result<(), AdvisorInputError> {
    let identity = match prepared {
        CatalogCombatStatSourceDispositionV1::Absent => {
            if let Some(captured) = captured {
                return Err(replay_source_mismatch(
                    battle_id,
                    player,
                    hand_slot,
                    source_kind,
                    format!(
                        "capture has id {}, catalog has no active source",
                        captured.id
                    ),
                ));
            }
            return Ok(());
        }
        // A Copy card's captured static block records what the Copy resolved to, never the
        // printed Copy itself, so capture and catalog identity cannot agree here. Refuse the
        // replay rather than preferring either side.
        CatalogCombatStatSourceDispositionV1::CopyOpponentSource { identity, .. } => {
            return Err(replay_source_mismatch(
                battle_id,
                player,
                hand_slot,
                source_kind,
                format!(
                    "catalog has unconditional Copy id {} ({:?}); a capture records only its resolved result",
                    identity.registry_definition_id, identity.description,
                ),
            ));
        }
        CatalogCombatStatSourceDispositionV1::Execute { identity, .. }
        | CatalogCombatStatSourceDispositionV1::ExecutePostRound { identity, .. } => identity,
    };
    let Some(captured) = captured else {
        return Err(replay_source_mismatch(
            battle_id,
            player,
            hand_slot,
            source_kind,
            format!(
                "capture has no active source, catalog has id {:?}",
                identity.catalog_id
            ),
        ));
    };
    if captured.description != identity.description {
        return Err(replay_source_mismatch(
            battle_id,
            player,
            hand_slot,
            source_kind,
            "capture and catalog descriptions differ".to_owned(),
        ));
    }
    let identity_matches = if require_catalog_identity {
        identity.catalog_id == Some(captured.id)
    } else {
        captured.id == identity.registry_definition_id
    };
    if !identity_matches {
        return Err(replay_source_mismatch(
            battle_id,
            player,
            hand_slot,
            source_kind,
            format!(
                "capture id {}, catalog id {:?}, registry definition {}, aliases {:?}",
                captured.id,
                identity.catalog_id,
                identity.registry_definition_id,
                identity.registry_alias_ids,
            ),
        ));
    }
    Ok(())
}

fn replay_source_mismatch(
    battle_id: u64,
    player: PlayerId,
    hand_slot: u8,
    source_kind: &'static str,
    detail: String,
) -> AdvisorInputError {
    AdvisorInputError::ReplaySourceMismatch {
        battle_id,
        player,
        hand_slot,
        source_kind,
        detail,
    }
}

const fn engine_player(player: EnginePlayer) -> PlayerId {
    match player {
        EnginePlayer::P1 => PlayerId::P1,
        EnginePlayer::P2 => PlayerId::P2,
    }
}

pub fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn player_input(
    hand: [CardKey; HAND_SIZE],
    initial_life: u16,
    initial_pillz: u16,
) -> CatalogCombatStatPlayerInputV1 {
    CatalogCombatStatPlayerInputV1 {
        initial_life,
        initial_pillz,
        hand,
    }
}

fn display_hand(
    hand: [CardKey; HAND_SIZE],
    catalog: &EffectiveCardCatalog,
) -> [AdvisorCardDisplay; HAND_SIZE] {
    hand.map(|key| {
        let card = catalog
            .get(key)
            .expect("strict catalog match already validated every display key");
        AdvisorCardDisplay {
            key,
            name: card.name.clone(),
            clan_name: card.clan_name.clone(),
            power: card.power,
            damage: card.damage,
        }
    })
}

fn mark_once(seen: &mut BTreeSet<String>, flag: &str) -> Result<(), AdvisorInputError> {
    if seen.insert(flag.to_owned()) {
        Ok(())
    } else {
        Err(argument_error(&format!("duplicate flag {flag}")))
    }
}

fn value_after<'a>(
    arguments: &'a [String],
    index: &mut usize,
    flag: &str,
) -> Result<&'a str, AdvisorInputError> {
    *index += 1;
    arguments
        .get(*index)
        .map(String::as_str)
        .ok_or_else(|| argument_error(&format!("{flag} requires a value")))
}

fn parse_hand(value: &str, flag: &str) -> Result<[CardKey; HAND_SIZE], AdvisorInputError> {
    let entries: Vec<&str> = value.split(',').collect();
    if entries.len() != HAND_SIZE || entries.iter().any(|entry| entry.is_empty()) {
        return Err(argument_error(&format!(
            "{flag} must contain exactly four id:level entries"
        )));
    }
    let mut keys = [CardKey::new(0, 0); HAND_SIZE];
    let mut ids = BTreeSet::new();
    for (slot, entry) in entries.into_iter().enumerate() {
        let mut fields = entry.split(':');
        let id = fields.next().unwrap();
        let level = fields.next();
        if fields.next().is_some() || level.is_none() || id.is_empty() {
            return Err(argument_error(&format!(
                "{flag} entry {} must be id:level",
                slot
            )));
        }
        let id = parse_nonzero(id, flag)?;
        let level: u8 = parse_nonzero(level.unwrap(), flag)?;
        if !ids.insert(id) {
            return Err(argument_error(&format!("{flag} repeats character id {id}")));
        }
        keys[slot] = CardKey::new(id, level);
    }
    Ok(keys)
}

fn parse_player(value: &str, flag: &str) -> Result<PlayerId, AdvisorInputError> {
    match value {
        "p1" => Ok(PlayerId::P1),
        "p2" => Ok(PlayerId::P2),
        _ => Err(argument_error(&format!(
            "{flag} must be p1 or p2, got {value:?}"
        ))),
    }
}

fn parse_second_card(value: &str) -> Result<u8, AdvisorInputError> {
    match value.parse::<u8>() {
        Ok(index) if index < HAND_SIZE as u8 => Ok(index),
        _ => Err(argument_error("--second-card must be an integer in 0..3")),
    }
}

fn parse_pillz(value: &str) -> Result<u16, AdvisorInputError> {
    match parse_unsigned::<u16>(value, "--pillz") {
        Ok(pillz) if pillz <= MAX_ADVISOR_PILLZ => Ok(pillz),
        Ok(_) => Err(argument_error(&format!(
            "--pillz must be at most {MAX_ADVISOR_PILLZ}"
        ))),
        Err(error) => Err(error),
    }
}

fn parse_nonzero<T>(value: &str, flag: &str) -> Result<T, AdvisorInputError>
where
    T: std::str::FromStr + PartialEq + From<u8>,
{
    match value.parse::<T>() {
        Ok(value) if value != T::from(0) => Ok(value),
        _ => Err(argument_error(&format!(
            "{flag} must be a positive integer"
        ))),
    }
}

fn parse_unsigned<T>(value: &str, flag: &str) -> Result<T, AdvisorInputError>
where
    T: std::str::FromStr,
{
    value
        .parse::<T>()
        .map_err(|_| argument_error(&format!("{flag} must be a non-negative integer")))
}

fn argument_error(message: &str) -> AdvisorInputError {
    AdvisorInputError::Argument(message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        load_replay, parse_args, prepare, repository_root, validate_replay_evidence,
        validate_replay_sources, AdvisorCommand, AdvisorInputError, AdvisorOptions,
        DEFAULT_BUDGET_MS,
    };
    use crate::catalog::{CardKey, EffectiveCardCatalog};
    use crate::engine::PlayerId;

    fn parse(arguments: &[&str]) -> AdvisorCommand {
        parse_args(arguments.iter().map(|argument| (*argument).to_owned())).unwrap()
    }

    #[test]
    fn defaults_to_the_proven_supported_demo_draw() {
        let AdvisorCommand::Run(options) = parse(&[]) else {
            panic!("empty argument list must run the demo");
        };
        assert!(options.using_demo_draw);
        assert_eq!(options.p1[0], CardKey::new(123, 1));
        assert_eq!(options.p2[3], CardKey::new(447, 1));
        assert_eq!(options.us, PlayerId::P1);
        assert_eq!(options.first_mover, PlayerId::P1);
        assert_eq!(options.budget_ms, DEFAULT_BUDGET_MS);
    }

    #[test]
    fn custom_draw_requires_both_hands_and_preserves_controls() {
        let AdvisorCommand::Run(options) = parse(&[
            "--p1",
            "1:1,2:2,3:3,4:4",
            "--p2",
            "5:1,6:2,7:3,8:4",
            "--life",
            "16",
            "--pillz",
            "9",
            "--night",
            "--us",
            "p2",
            "--first",
            "p1",
            "--second-card",
            "3",
            "--budget-ms",
            "25",
            "--width",
            "100",
            "--height",
            "40",
            "--plain",
        ]) else {
            panic!("custom invocation must run");
        };
        assert!(!options.using_demo_draw);
        assert_eq!(options.p1[1], CardKey::new(2, 2));
        assert_eq!(options.p2[2], CardKey::new(7, 3));
        assert_eq!(options.life, 16);
        assert_eq!(options.pillz, 9);
        assert!(options.night);
        assert_eq!(options.us, PlayerId::P2);
        assert_eq!(options.first_mover, PlayerId::P1);
        assert_eq!(options.second_card, Some(3));
        assert!(options.plain);
    }

    #[test]
    fn replay_mode_accepts_only_budget_and_display_controls() {
        let AdvisorCommand::Run(options) = parse(&[
            "--replay",
            "877636",
            "--budget-ms",
            "25",
            "--width",
            "100",
            "--height",
            "40",
            "--plain",
        ]) else {
            panic!("replay invocation must run");
        };
        assert_eq!(options.replay_id, Some(877636));
        assert!(!options.using_demo_draw);
        assert_eq!(options.budget_ms, 25);
        assert!(options.plain);

        for conflict in [
            "--demo",
            "--p1",
            "--p2",
            "--life",
            "--pillz",
            "--night",
            "--us",
            "--first",
            "--second-card",
            "--interactive",
        ] {
            let mut arguments = vec!["--replay", "877636", conflict];
            if matches!(
                conflict,
                "--p1" | "--p2" | "--life" | "--pillz" | "--us" | "--first" | "--second-card"
            ) {
                arguments.push(if matches!(conflict, "--us" | "--first") {
                    "p1"
                } else if matches!(conflict, "--p1" | "--p2") {
                    "1:1,2:1,3:1,4:1"
                } else {
                    "1"
                });
            }
            assert!(
                parse_args(arguments.into_iter().map(str::to_owned)).is_err(),
                "{conflict}"
            );
        }
    }

    #[test]
    fn replay_preparation_requires_capture_and_catalog_source_identity_to_agree() {
        let AdvisorCommand::Run(options) = parse(&["--replay", "877636", "--plain"]) else {
            panic!("the replay invocation must run");
        };
        let prepared = prepare(options).expect("the audited capture must be fully executable");
        let root = repository_root();
        let catalog = EffectiveCardCatalog::load(
            root.join("data/data.json"),
            root.join("data/battle_card_overrides.json"),
        )
        .unwrap();
        let mut replay = load_replay(&root, 877636, &catalog).unwrap();
        // Dave's printed catalog ability is id 888. Registry id 401 has the same structure
        // and text, but a captured dynamic replacement must not inherit printed authority.
        replay.players[1].hand[1]
            .source_ability
            .as_mut()
            .unwrap()
            .id = 401;
        assert!(matches!(
            validate_replay_sources(&replay, &prepared.combat_match),
            Err(AdvisorInputError::ReplaySourceMismatch {
                player: PlayerId::P2,
                hand_slot: 1,
                source_kind: "Ability",
                ..
            })
        ));

        let mut replay = load_replay(&root, 877636, &catalog).unwrap();
        // Ulu Watu's executable bonus is registry definition 39. Id 43 has the same
        // description, but an alias is provenance rather than execution authority.
        replay.players[1].hand[0].source_bonus.as_mut().unwrap().id = 43;
        assert!(matches!(
            validate_replay_sources(&replay, &prepared.combat_match),
            Err(AdvisorInputError::ReplaySourceMismatch {
                player: PlayerId::P2,
                hand_slot: 0,
                source_kind: "Bonus",
                ..
            })
        ));

        let mut replay = load_replay(&root, 877636, &catalog).unwrap();
        replay.players[1].hand[1]
            .source_ability
            .as_mut()
            .unwrap()
            .description = "\u{1b}[31mforged\r\nsource".to_owned();
        let error = validate_replay_sources(&replay, &prepared.combat_match).unwrap_err();
        let rendered = error.to_string();
        assert!(!rendered.contains('\u{1b}'));
        assert!(!rendered.contains('\r'));
        assert!(!rendered.contains('\n'));
        assert!(rendered.contains("descriptions differ"));
    }

    #[test]
    fn strict_replay_advice_rejects_missing_server_card_evidence() {
        let root = repository_root();
        let catalog = EffectiveCardCatalog::load(
            root.join("data/data.json"),
            root.join("data/battle_card_overrides.json"),
        )
        .unwrap();
        let mut replay = load_replay(&root, 877636, &catalog).unwrap();
        replay.rounds[2].expected_card_results[1] = None;
        assert!(matches!(
            validate_replay_evidence(&replay),
            Err(AdvisorInputError::ReplayEvidenceMissing {
                battle_id: 877636,
                round: 2,
                player: PlayerId::P2,
            })
        ));
    }

    #[test]
    fn rejects_ambiguous_or_malformed_invocations() {
        for arguments in [
            vec!["--p1", "1:1,2:1,3:1,4:1"],
            vec![
                "--demo",
                "--p1",
                "1:1,2:1,3:1,4:1",
                "--p2",
                "5:1,6:1,7:1,8:1",
            ],
            vec!["--life", "12", "--life", "13"],
            vec!["--second-card", "4", "--us", "p2"],
            vec!["--second-card", "0"],
            vec!["--us", "p2"],
            vec!["--unknown"],
            vec!["--p1", "1:1,2:1,3:1"],
            vec!["--help", "--plain"],
        ] {
            assert!(parse_args(arguments.into_iter().map(str::to_owned)).is_err());
        }
    }

    #[test]
    fn help_is_a_non_error_command() {
        assert!(matches!(parse(&["--help"]), AdvisorCommand::Help));
        assert_eq!(AdvisorOptions::default().life, 14);
    }

    #[test]
    fn accepts_a_zero_pillz_current_position() {
        let AdvisorCommand::Run(options) = parse(&["--pillz", "0"]) else {
            panic!("zero pillz is a valid current position");
        };
        assert_eq!(options.pillz, 0);
    }

    #[test]
    fn interactive_mode_prompts_for_second_mover_information() {
        let AdvisorCommand::Run(options) = parse(&["--interactive", "--us", "p2", "--first", "p1"])
        else {
            panic!("interactive second-mover mode must not need a one-shot revealed card");
        };
        assert!(options.interactive);
        assert_eq!(options.second_card, None);
        assert!(parse_args(
            ["--interactive", "--second-card", "0"]
                .into_iter()
                .map(str::to_owned)
        )
        .is_err());
    }

    #[test]
    fn rejects_a_pillz_pool_large_enough_to_exhaust_the_search_matrix() {
        assert!(parse_args(["--pillz", "65535"].into_iter().map(str::to_owned)).is_err());
        let AdvisorCommand::Run(options) = parse(&["--pillz", "255"]) else {
            panic!("the documented upper bound remains usable");
        };
        assert_eq!(options.pillz, 255);
    }
}
