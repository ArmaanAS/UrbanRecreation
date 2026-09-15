//! A normalized execution/assertion view of a captured game.
//!
//! `ReplayCaseV1` is intentionally not a lossless replacement for the source capture. The
//! original file remains the provenance record for timing, post-round effects, and final
//! rewards outside the engine execution assertions represented here.

use crate::catalog::CardKey;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

pub const REPLAY_SCHEMA_VERSION: u16 = 1;

/// A source-side identifier, retained even after players are put in engine order.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceSide {
    Side0,
    Side1,
}

impl SourceSide {
    pub const fn other(self) -> Self {
        match self {
            Self::Side0 => Self::Side1,
            Self::Side1 => Self::Side0,
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Side0 => 0,
            Self::Side1 => 1,
        }
    }
}

/// The normalized player identity expected by both engines.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnginePlayer {
    P1,
    P2,
}

impl EnginePlayer {
    pub const fn other(self) -> Self {
        match self {
            Self::P1 => Self::P2,
            Self::P2 => Self::P1,
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::P1 => 0,
            Self::P2 => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    Done,
    Timeout,
    Left,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceRoom {
    pub id: u64,
    pub name: String,
    pub battle_rule_id: u32,
    pub deck_format_id: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReplayMetadata {
    pub battle_id: u64,
    pub captured_at: String,
    pub creation_time: u64,
    pub room: Option<SourceRoom>,
    pub battle_rule_id: u32,
    pub night: bool,
    pub source_status: SourceStatus,
    pub source_first_side: SourceSide,
    pub recording_player_id: Option<u64>,
    pub recording_side: Option<SourceSide>,
    pub source_snapshot_count: u32,
    pub source_issues: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceClub {
    pub id: u64,
    pub name: String,
}

/// Profile fields from the capture, kept separately from engine resources.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceProfile {
    pub id: u64,
    pub name: String,
    pub level: u16,
    pub grade: String,
    pub country: String,
    pub club: Option<SourceClub>,
    pub registration_time: u64,
    pub certification_level: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceModifier {
    pub id: u32,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReplayCard {
    pub key: CardKey,
    pub hand_index: u8,
    pub source_name: String,
    pub source_clan: String,
    pub in_battle_id: u32,
    pub state: String,
    pub source_ability: Option<SourceModifier>,
    pub source_bonus: Option<SourceModifier>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReplayPlayer {
    pub engine_player: EnginePlayer,
    pub source_side: SourceSide,
    pub profile: SourceProfile,
    pub base_life: u16,
    pub base_pillz: u16,
    /// Cards are always ordered by `hand_index`, which is the set `0..4`.
    pub hand: [ReplayCard; 4],
}

/// A selection in the order it was actually submitted during the round.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReplayPlay {
    pub engine_player: EnginePlayer,
    pub source_side: SourceSide,
    pub hand_index: u8,
    pub card: CardKey,
    /// Engine convention: excludes the free pill and the three fury pillz.
    pub pillz: u16,
    pub source_pillz_used: u16,
    pub fury: bool,
    pub source_timestamp_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExpectedPlayerState {
    pub life: u16,
    pub pillz: u16,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExpectedCardResult {
    pub power: u16,
    /// The transient damage reported at resolution, before later snapshots revert it.
    pub damage: u16,
    pub attack: u32,
    pub won: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReplayRound {
    pub round: u8,
    pub first_mover: EnginePlayer,
    pub source_first_side: SourceSide,
    /// Actual source move order, first mover followed by second mover.
    pub plays: [ReplayPlay; 2],
    /// Engine P1 followed by engine P2.
    pub expected_player_states: [ExpectedPlayerState; 2],
    /// Engine P1 followed by engine P2. Old captures may lack card results.
    pub expected_card_results: [Option<ExpectedCardResult>; 2],
}

/// Versioned, engine-independent replay input and expected output.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReplayCaseV1 {
    #[serde(deserialize_with = "deserialize_schema_version")]
    pub schema_version: u16,
    pub metadata: ReplayMetadata,
    /// Normalized order: engine P1 is the source's first player, then engine P2.
    pub players: [ReplayPlayer; 2],
    /// Only the fully resolved prefix represented by the embedded testcase.
    pub rounds: Vec<ReplayRound>,
}

fn deserialize_schema_version<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u16::deserialize(deserializer)?;
    if version == REPLAY_SCHEMA_VERSION {
        Ok(version)
    } else {
        Err(D::Error::custom(format!(
            "unsupported replay schema version {version}; expected {REPLAY_SCHEMA_VERSION}"
        )))
    }
}
