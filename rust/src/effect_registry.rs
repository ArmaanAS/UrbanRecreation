//! Validated, versioned compilation boundary for the captured effect dictionary.
//!
//! Loading is deliberately separate from execution. The registry preserves the complete
//! structured source record, compiles only evidence-backed shapes, and represents every
//! other well-formed shape as [`CompiledEffectV1::Unsupported`]. It never turns an unknown
//! effect into a successful no-op. The boundary is replay-model neutral: callers resolve
//! their own source records through strict id-and-description lookup.

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

pub const EFFECT_REGISTRY_SCHEMA_VERSION: u16 = 1;

macro_rules! string_enum {
    ($(#[$meta:meta])* pub enum $name:ident { $($variant:ident => $source:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const DOMAIN: &'static [&'static str] = &[$($source),+];

            fn parse(
                effect_id: u32,
                field: &'static str,
                value: String,
            ) -> Result<Self, EffectRegistryError> {
                match value.as_str() {
                    $($source => Ok(Self::$variant),)+
                    _ => Err(EffectRegistryError::UnknownEnumValue {
                        effect_id,
                        field,
                        value,
                        expected: Self::DOMAIN,
                    }),
                }
            }
        }
    };
}

string_enum! {
    pub enum PositionRequirementV1 {
        Attacker => "attacker",
        Both => "both",
        Defender => "defender",
    }
}

string_enum! {
    pub enum PreviousRoundRequirementV1 {
        Any => "any",
        Lose => "lose",
        Win => "win",
    }
}

string_enum! {
    pub enum CurrentRoundRequirementV1 {
        Any => "any",
        Lose => "lose",
        Perfect => "perfect",
        Sureshot => "sureshot",
        Win => "win",
    }
}

string_enum! {
    pub enum IndexRequirementV1 {
        Any => "any",
        Asymmetry => "asymmetry",
        Symmetry => "symmetry",
    }
}

string_enum! {
    pub enum BetPillzLinkV1 {
        Less => "less",
        More => "more",
        No => "no",
    }
}

string_enum! {
    pub enum AffectedSideV1 {
        Both => "both",
        Opponent => "opponent",
        Player => "player",
    }
}

string_enum! {
    pub enum AttributeAffectedV1 {
        Attack => "atk",
        Damage => "dmg",
        Life => "life",
        LifeAndPillz => "life&pillz",
        None => "none",
        Pillz => "pillz",
        Power => "pwr",
        PowerAndAttack => "pwr&atk",
        PowerAndDamage => "pwr&dmg",
    }
}

string_enum! {
    pub enum AttributeActionV1 {
        Copy => "copy",
        Decrease => "decrease",
        Increase => "increase",
        None => "none",
        Protect => "protect",
        Simplify => "simplify",
        StopModifier => "stop_modif",
    }
}

string_enum! {
    pub enum SpecialActionV1 {
        ConvertDamageToLife => "convert_dmg_to_life",
        ConvertDamageToPillz => "convert_dmg_to_pillz",
        ConvertOpponentDamageToAttack => "convert_opp_dmg_to_atk",
        ConvertOpponentDamageToLife => "convert_opp_dmg_to_life",
        ConvertOpponentPowerToAttack => "convert_opp_pwr_to_atk",
        CopyAbility => "copy_ability",
        CopyBonus => "copy_bonus",
        Knockout => "ko",
        Mock => "mock",
        None => "none",
        ProtectAbility => "protect_ability",
        ProtectBonus => "protect_bonus",
        RandomAbilities => "random_abilities",
        RecoverPillz => "recover_pillz",
        StopAbility => "stop_ability",
        StopBonus => "stop_bonus",
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ClanIdsV1(Box<[u32]>);

impl ClanIdsV1 {
    pub fn as_slice(&self) -> &[u32] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The complete typed `abilityData` input as captured from the site.
///
/// It is intentionally not deserializable directly: callers must go through
/// [`EffectRegistryV1`] so enum domains and structural invariants are validated.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StructuredEffectV1 {
    pub value: u16,
    pub value_min: u16,
    pub value_max: u16,
    pub value_condition: u16,
    pub position_requirement: PositionRequirementV1,
    pub previous_round_requirement: PreviousRoundRequirementV1,
    pub current_round_requirement: CurrentRoundRequirementV1,
    pub index_requirement: IndexRequirementV1,
    pub clan_requirement: ClanIdsV1,
    pub opponent_clan_requirement: ClanIdsV1,
    pub previous_clan_requirement: ClanIdsV1,
    pub bet_pillz_link: BetPillzLinkV1,
    pub side_affected: AffectedSideV1,
    pub attribute_affected: AttributeAffectedV1,
    pub attribute_action: AttributeActionV1,
    pub special_action: SpecialActionV1,
    pub is_inverted: bool,
    pub is_support: bool,
    pub is_anti_support: bool,
    pub is_overdrive: bool,
    pub is_divide: bool,
    pub is_life_linked: bool,
    pub is_pillz_linked: bool,
    pub is_lost_life_linked: bool,
    pub is_lost_pillz_linked: bool,
    pub is_opponent_stars_linked: bool,
    pub is_clanmates_count_linked: bool,
    pub is_anti_clanmates_count_linked: bool,
    pub is_permanent: bool,
    pub is_immediate_permanent: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CombatStatV1 {
    Attack,
    Damage,
    Power,
    PowerAndDamage,
}

impl CombatStatV1 {
    fn from_attribute(attribute: AttributeAffectedV1) -> Option<Self> {
        match attribute {
            AttributeAffectedV1::Attack => Some(Self::Attack),
            AttributeAffectedV1::Damage => Some(Self::Damage),
            AttributeAffectedV1::Power => Some(Self::Power),
            AttributeAffectedV1::PowerAndDamage => Some(Self::PowerAndDamage),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatOperationV1 {
    Decrease,
    Increase,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MagnitudeMultiplierV1 {
    Fixed,
    Support,
    Growth,
    Degrowth,
    OpponentStars,
}

/// Compact, string-free building blocks safe to copy into later round plans.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SupportedEffectV1 {
    ModifyCombatStat {
        side: AffectedSideV1,
        stat: CombatStatV1,
        operation: StatOperationV1,
        value: u16,
        minimum: Option<u16>,
        maximum: Option<u16>,
        multiplier: MagnitudeMultiplierV1,
    },
    StopOpponentAbility,
    StopOpponentBonus,
    CancelOpponentCombatStatModifiers {
        stat: CombatStatV1,
    },
    /// The owner's own stat cannot be reduced by the opposing character.
    ProtectOwnCombatStat {
        stat: CombatStatV1,
    },
    /// The owner's own Ability cannot be stopped by the opposing character.
    ProtectOwnAbility,
    /// The owner's own Bonus cannot be stopped by the opposing character.
    ProtectOwnBonus,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionalFeatureV1 {
    BetPillz,
    Clan,
    CurrentRound,
    Index,
    OpponentClan,
    Position,
    PreviousClan,
    PreviousRound,
    ValueCondition,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkedMagnitudeV1 {
    AntiClanmatesCount,
    AntiSupport,
    ClanmatesCount,
    Divide,
    Life,
    LostLife,
    LostPillz,
    OpponentStars,
    Overdrive,
    Pillz,
    Support,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DescriptionContextV1 {
    Cards,
    Day,
    Night,
    OtherUnreviewedGrammar,
    Team,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum UnsupportedReasonV1 {
    Attribute { attribute: AttributeAffectedV1 },
    AttributeAction { action: AttributeActionV1 },
    Conditional { feature: ConditionalFeatureV1 },
    DescriptionContext { context: DescriptionContextV1 },
    ImmediatePermanent,
    Inverted,
    LinkedMagnitude { link: LinkedMagnitudeV1 },
    Permanent,
    SpecialAction { action: SpecialActionV1 },
    UnsupportedSide { side: AffectedSideV1 },
    NonZeroControlValues,
    IncompatibleBounds,
    ZeroMagnitude,
    UnrecognizedShape,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "classification", content = "value")]
pub enum CompiledEffectV1 {
    Supported(SupportedEffectV1),
    Unsupported(Box<[UnsupportedReasonV1]>),
}

impl CompiledEffectV1 {
    pub const fn supported(&self) -> Option<SupportedEffectV1> {
        match self {
            Self::Supported(effect) => Some(*effect),
            Self::Unsupported(_) => None,
        }
    }

    pub fn unsupported_reasons(&self) -> &[UnsupportedReasonV1] {
        match self {
            Self::Supported(_) => &[],
            Self::Unsupported(reasons) => reasons,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EffectDefinitionV1 {
    id: u32,
    unlock_level: u16,
    description: String,
    long_description: String,
    structured_input: StructuredEffectV1,
    compiled: CompiledEffectV1,
}

impl EffectDefinitionV1 {
    pub const fn id(&self) -> u32 {
        self.id
    }

    pub const fn unlock_level(&self) -> u16 {
        self.unlock_level
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn long_description(&self) -> &str {
        &self.long_description
    }

    pub const fn structured_input(&self) -> &StructuredEffectV1 {
        &self.structured_input
    }

    pub const fn compiled(&self) -> &CompiledEffectV1 {
        &self.compiled
    }
}

/// FNV-1a over the exact source bytes supplied to the loader.
///
/// This is a deterministic change detector, not a cryptographic content hash.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct SourceFingerprintFnv1a64(u64);

impl SourceFingerprintFnv1a64 {
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for SourceFingerprintFnv1a64 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "fnv1a64:{:016x}", self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DescriptionMatchV1<'a> {
    definition: &'a EffectDefinitionV1,
    alias_ids: &'a [u32],
}

impl<'a> DescriptionMatchV1<'a> {
    pub const fn definition(self) -> &'a EffectDefinitionV1 {
        self.definition
    }

    /// Every capture-registry id with the same exact description and typed structure.
    pub const fn alias_ids(self) -> &'a [u32] {
        self.alias_ids
    }
}

#[derive(Clone, Debug)]
pub struct EffectRegistryV1 {
    schema_version: u16,
    source_fingerprint_fnv1a64: SourceFingerprintFnv1a64,
    by_id: BTreeMap<u32, EffectDefinitionV1>,
    ids_by_description: BTreeMap<String, Vec<u32>>,
}

impl EffectRegistryV1 {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, EffectRegistryError> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|source| EffectRegistryError::Open {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_reader(file)
    }

    pub fn from_reader(mut reader: impl Read) -> Result<Self, EffectRegistryError> {
        let mut source_bytes = Vec::new();
        reader
            .read_to_end(&mut source_bytes)
            .map_err(EffectRegistryError::Read)?;
        let source_fingerprint_fnv1a64 = SourceFingerprintFnv1a64(fnv1a64(&source_bytes));
        let RawRegistry(entries) =
            serde_json::from_slice(&source_bytes).map_err(EffectRegistryError::Parse)?;
        if entries.is_empty() {
            return Err(EffectRegistryError::EmptyRegistry);
        }

        let mut textual_keys = BTreeSet::new();
        let mut numeric_keys: BTreeMap<u32, String> = BTreeMap::new();
        let mut by_id = BTreeMap::new();
        let mut ids_by_description: BTreeMap<String, Vec<u32>> = BTreeMap::new();

        for (map_key, raw) in entries {
            if !textual_keys.insert(map_key.clone()) {
                return Err(EffectRegistryError::DuplicateMapKey { key: map_key });
            }
            let key_id =
                map_key
                    .parse::<u32>()
                    .map_err(|_| EffectRegistryError::InvalidMapKey {
                        key: map_key.clone(),
                    })?;
            if let Some(first_key) = numeric_keys.insert(key_id, map_key.clone()) {
                return Err(EffectRegistryError::NumericKeyAlias {
                    effect_id: key_id,
                    first_key,
                    second_key: map_key,
                });
            }
            if key_id != raw.id {
                return Err(EffectRegistryError::EmbeddedIdMismatch {
                    map_key,
                    map_key_id: key_id,
                    embedded_id: raw.id,
                });
            }

            let input = StructuredEffectV1::try_from_raw(raw.id, raw.ability_data)?;
            validate_structure(raw.id, &input)?;
            let compiled = compile(&input, &raw.description);
            let definition = EffectDefinitionV1 {
                id: raw.id,
                unlock_level: raw.unlock_level,
                description: raw.description,
                long_description: raw.long_description,
                structured_input: input,
                compiled,
            };
            ids_by_description
                .entry(definition.description.clone())
                .or_default()
                .push(definition.id);
            by_id.insert(definition.id, definition);
        }
        for ids in ids_by_description.values_mut() {
            ids.sort_unstable();
        }

        Ok(Self {
            schema_version: EFFECT_REGISTRY_SCHEMA_VERSION,
            source_fingerprint_fnv1a64,
            by_id,
            ids_by_description,
        })
    }

    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    pub const fn source_fingerprint_fnv1a64(&self) -> SourceFingerprintFnv1a64 {
        self.source_fingerprint_fnv1a64
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn get(&self, id: u32) -> Option<&EffectDefinitionV1> {
        self.by_id.get(&id)
    }

    /// Iterates definitions in ascending numeric id order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (u32, &EffectDefinitionV1)> {
        self.by_id.iter().map(|(&id, definition)| (id, definition))
    }

    /// Strict capture lookup: both the stable id and captured description must agree.
    pub fn lookup_capture(
        &self,
        id: u32,
        description: &str,
    ) -> Result<&EffectDefinitionV1, EffectLookupError> {
        let definition = self
            .by_id
            .get(&id)
            .ok_or(EffectLookupError::MissingId { id })?;
        if definition.description != description {
            return Err(EffectLookupError::DescriptionMismatch {
                id,
                expected: definition.description.clone(),
                actual: description.to_owned(),
            });
        }
        Ok(definition)
    }

    /// Exact-description fallback for catalog rows whose legacy numeric id differs.
    ///
    /// Structurally identical aliases are returned as one deterministic match with every
    /// alias id exposed. A shared description with conflicting structured data is rejected.
    pub fn lookup_description(
        &self,
        description: &str,
    ) -> Result<DescriptionMatchV1<'_>, EffectLookupError> {
        let ids = self.ids_by_description.get(description).ok_or_else(|| {
            EffectLookupError::MissingDescription {
                description: description.to_owned(),
            }
        })?;
        let first = &self.by_id[&ids[0]];
        if ids
            .iter()
            .skip(1)
            .any(|id| self.by_id[id].structured_input != first.structured_input)
        {
            return Err(EffectLookupError::AmbiguousDescription {
                description: description.to_owned(),
                ids: ids.clone(),
            });
        }
        Ok(DescriptionMatchV1 {
            definition: first,
            alias_ids: ids,
        })
    }
}

#[derive(Debug)]
pub enum EffectRegistryError {
    Open {
        path: PathBuf,
        source: io::Error,
    },
    Read(io::Error),
    Parse(serde_json::Error),
    EmptyRegistry,
    InvalidMapKey {
        key: String,
    },
    DuplicateMapKey {
        key: String,
    },
    NumericKeyAlias {
        effect_id: u32,
        first_key: String,
        second_key: String,
    },
    EmbeddedIdMismatch {
        map_key: String,
        map_key_id: u32,
        embedded_id: u32,
    },
    UnknownEnumValue {
        effect_id: u32,
        field: &'static str,
        value: String,
        expected: &'static [&'static str],
    },
    InvalidClanId {
        effect_id: u32,
        field: &'static str,
        value: String,
    },
    InvalidCombination {
        effect_id: u32,
        field: &'static str,
        reason: &'static str,
    },
}

impl fmt::Display for EffectRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { path, source } => {
                write!(formatter, "failed to open {}: {source}", path.display())
            }
            Self::Read(source) => write!(formatter, "failed to read effect registry: {source}"),
            Self::Parse(source) => write!(formatter, "failed to parse effect registry: {source}"),
            Self::EmptyRegistry => write!(formatter, "effect registry is empty"),
            Self::InvalidMapKey { key } => write!(formatter, "effect map key {key:?} is not u32"),
            Self::DuplicateMapKey { key } => write!(formatter, "duplicate effect map key {key:?}"),
            Self::NumericKeyAlias {
                effect_id,
                first_key,
                second_key,
            } => write!(
                formatter,
                "effect id {effect_id} has aliased map keys {first_key:?} and {second_key:?}"
            ),
            Self::EmbeddedIdMismatch {
                map_key,
                map_key_id,
                embedded_id,
            } => write!(
                formatter,
                "effect map key {map_key:?} ({map_key_id}) does not match embedded id {embedded_id}"
            ),
            Self::UnknownEnumValue {
                effect_id,
                field,
                value,
                expected,
            } => write!(
                formatter,
                "effect {effect_id} field {field} has unknown value {value:?}; expected one of {}",
                expected.join(", ")
            ),
            Self::InvalidClanId {
                effect_id,
                field,
                value,
            } => write!(
                formatter,
                "effect {effect_id} field {field} contains invalid clan id {value:?}"
            ),
            Self::InvalidCombination {
                effect_id,
                field,
                reason,
            } => write!(
                formatter,
                "effect {effect_id} field {field} forms an invalid combination: {reason}"
            ),
        }
    }
}

impl Error for EffectRegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Open { source, .. } | Self::Read(source) => Some(source),
            Self::Parse(source) => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectLookupError {
    MissingId {
        id: u32,
    },
    DescriptionMismatch {
        id: u32,
        expected: String,
        actual: String,
    },
    MissingDescription {
        description: String,
    },
    AmbiguousDescription {
        description: String,
        ids: Vec<u32>,
    },
}

impl fmt::Display for EffectLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingId { id } => write!(formatter, "effect id {id} is missing"),
            Self::DescriptionMismatch {
                id,
                expected,
                actual,
            } => write!(
                formatter,
                "effect id {id} description mismatch: expected {expected:?}, got {actual:?}"
            ),
            Self::MissingDescription { description } => {
                write!(formatter, "effect description {description:?} is missing")
            }
            Self::AmbiguousDescription { description, ids } => write!(
                formatter,
                "effect description {description:?} has conflicting ids {ids:?}"
            ),
        }
    }
}

impl Error for EffectLookupError {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
struct RawEffect {
    id: u32,
    unlock_level: u16,
    description: String,
    long_description: String,
    ability_data: RawStructuredEffect,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
struct RawStructuredEffect {
    value: u16,
    value_min: u16,
    value_max: u16,
    value_condition: u16,
    position_requirement: String,
    previous_round_requirement: String,
    current_round_requirement: String,
    index_requirement: String,
    clan_requirement: RawClanRequirement,
    opp_clan_requirement: RawClanRequirement,
    previous_clan_requirement: RawClanRequirement,
    bet_pillz_link: String,
    side_affected: String,
    attribute_affected: String,
    attribute_action: String,
    special_action: String,
    is_inverted: bool,
    is_support: bool,
    is_anti_support: bool,
    is_overdrive: bool,
    is_divide: bool,
    is_life_linked: bool,
    is_pillz_linked: bool,
    is_lost_life_linked: bool,
    is_lost_pillz_linked: bool,
    is_opp_stars_linked: bool,
    is_clanmates_count_linked: bool,
    is_anti_clanmates_count_linked: bool,
    is_permanent: bool,
    is_immediate_permanent: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawClanRequirement {
    Text(String),
    One(u32),
}

struct RawRegistry(Vec<(String, RawEffect)>);

impl<'de> Deserialize<'de> for RawRegistry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RegistryVisitor;

        impl<'de> Visitor<'de> for RegistryVisitor {
            type Value = RawRegistry;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object mapping numeric effect ids to effect entries")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut entries = Vec::with_capacity(map.size_hint().unwrap_or(0));
                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }
                Ok(RawRegistry(entries))
            }
        }

        deserializer.deserialize_map(RegistryVisitor)
    }
}

impl StructuredEffectV1 {
    fn try_from_raw(effect_id: u32, raw: RawStructuredEffect) -> Result<Self, EffectRegistryError> {
        Ok(Self {
            value: raw.value,
            value_min: raw.value_min,
            value_max: raw.value_max,
            value_condition: raw.value_condition,
            position_requirement: PositionRequirementV1::parse(
                effect_id,
                "positionRequirement",
                raw.position_requirement,
            )?,
            previous_round_requirement: PreviousRoundRequirementV1::parse(
                effect_id,
                "previousRoundRequirement",
                raw.previous_round_requirement,
            )?,
            current_round_requirement: CurrentRoundRequirementV1::parse(
                effect_id,
                "currentRoundRequirement",
                raw.current_round_requirement,
            )?,
            index_requirement: IndexRequirementV1::parse(
                effect_id,
                "indexRequirement",
                raw.index_requirement,
            )?,
            clan_requirement: parse_clan_ids(effect_id, "clanRequirement", raw.clan_requirement)?,
            opponent_clan_requirement: parse_clan_ids(
                effect_id,
                "oppClanRequirement",
                raw.opp_clan_requirement,
            )?,
            previous_clan_requirement: parse_clan_ids(
                effect_id,
                "previousClanRequirement",
                raw.previous_clan_requirement,
            )?,
            bet_pillz_link: BetPillzLinkV1::parse(effect_id, "betPillzLink", raw.bet_pillz_link)?,
            side_affected: AffectedSideV1::parse(effect_id, "sideAffected", raw.side_affected)?,
            attribute_affected: AttributeAffectedV1::parse(
                effect_id,
                "attributeAffected",
                raw.attribute_affected,
            )?,
            attribute_action: AttributeActionV1::parse(
                effect_id,
                "attributeAction",
                raw.attribute_action,
            )?,
            special_action: SpecialActionV1::parse(effect_id, "specialAction", raw.special_action)?,
            is_inverted: raw.is_inverted,
            is_support: raw.is_support,
            is_anti_support: raw.is_anti_support,
            is_overdrive: raw.is_overdrive,
            is_divide: raw.is_divide,
            is_life_linked: raw.is_life_linked,
            is_pillz_linked: raw.is_pillz_linked,
            is_lost_life_linked: raw.is_lost_life_linked,
            is_lost_pillz_linked: raw.is_lost_pillz_linked,
            is_opponent_stars_linked: raw.is_opp_stars_linked,
            is_clanmates_count_linked: raw.is_clanmates_count_linked,
            is_anti_clanmates_count_linked: raw.is_anti_clanmates_count_linked,
            is_permanent: raw.is_permanent,
            is_immediate_permanent: raw.is_immediate_permanent,
        })
    }
}

fn parse_clan_ids(
    effect_id: u32,
    field: &'static str,
    source: RawClanRequirement,
) -> Result<ClanIdsV1, EffectRegistryError> {
    let source = match source {
        RawClanRequirement::Text(source) => source,
        RawClanRequirement::One(id) => return Ok(ClanIdsV1(vec![id].into_boxed_slice())),
    };
    if source.is_empty() {
        return Ok(ClanIdsV1::default());
    }
    let mut ids = Vec::new();
    for value in source.split(',') {
        let id = value
            .parse::<u32>()
            .map_err(|_| EffectRegistryError::InvalidClanId {
                effect_id,
                field,
                value: value.to_owned(),
            })?;
        ids.push(id);
    }
    Ok(ClanIdsV1(ids.into_boxed_slice()))
}

fn validate_structure(
    effect_id: u32,
    input: &StructuredEffectV1,
) -> Result<(), EffectRegistryError> {
    if input.is_support && input.is_anti_support {
        return Err(EffectRegistryError::InvalidCombination {
            effect_id,
            field: "isSupport/isAntiSupport",
            reason: "support and anti-support cannot both be true",
        });
    }
    if input.is_clanmates_count_linked && input.is_anti_clanmates_count_linked {
        return Err(EffectRegistryError::InvalidCombination {
            effect_id,
            field: "isClanmatesCountLinked/isAntiClanmatesCountLinked",
            reason: "clanmates and anti-clanmates count links cannot both be true",
        });
    }
    if input.is_immediate_permanent && !input.is_permanent {
        return Err(EffectRegistryError::InvalidCombination {
            effect_id,
            field: "isImmediatePermanent",
            reason: "immediate permanent requires isPermanent",
        });
    }
    if input.special_action == SpecialActionV1::None
        && (input.attribute_affected == AttributeAffectedV1::None)
            != (input.attribute_action == AttributeActionV1::None)
    {
        return Err(EffectRegistryError::InvalidCombination {
            effect_id,
            field: "attributeAffected/attributeAction",
            reason: "without a special action, attribute and action must either both be none or both be set",
        });
    }
    if input.attribute_action == AttributeActionV1::StopModifier
        && (input.value != 0
            || input.value_min != 0
            || input.value_max != 0
            || input.value_condition != 0)
    {
        return Err(EffectRegistryError::InvalidCombination {
            effect_id,
            field: "attributeAction",
            reason: "stop_modif cannot carry magnitude or control values",
        });
    }
    Ok(())
}

fn compile(input: &StructuredEffectV1, description: &str) -> CompiledEffectV1 {
    let mut reasons = common_unsupported_reasons(input);
    if let Some(context) = explicit_description_context(description) {
        reasons.insert(UnsupportedReasonV1::DescriptionContext { context });
    }

    let candidate = match (
        input.attribute_action,
        input.special_action,
        CombatStatV1::from_attribute(input.attribute_affected),
    ) {
        (AttributeActionV1::Increase, SpecialActionV1::None, Some(stat))
        | (AttributeActionV1::Decrease, SpecialActionV1::None, Some(stat)) => {
            if input.value == 0 {
                reasons.insert(UnsupportedReasonV1::ZeroMagnitude);
                None
            } else if (input.attribute_action == AttributeActionV1::Increase
                && input.value_min != 0)
                || (input.attribute_action == AttributeActionV1::Decrease && input.value_max != 0)
            {
                reasons.insert(UnsupportedReasonV1::IncompatibleBounds);
                None
            } else {
                let operation = if input.attribute_action == AttributeActionV1::Increase {
                    StatOperationV1::Increase
                } else {
                    StatOperationV1::Decrease
                };
                Some(SupportedEffectV1::ModifyCombatStat {
                    side: input.side_affected,
                    stat,
                    operation,
                    value: input.value,
                    minimum: (operation == StatOperationV1::Decrease).then_some(input.value_min),
                    maximum: (input.value_max != 0).then_some(input.value_max),
                    multiplier: if input.is_support {
                        MagnitudeMultiplierV1::Support
                    } else {
                        MagnitudeMultiplierV1::Fixed
                    },
                })
            }
        }
        (AttributeActionV1::None, SpecialActionV1::StopAbility, _)
            if input.attribute_affected == AttributeAffectedV1::None =>
        {
            if input.side_affected != AffectedSideV1::Player {
                reasons.insert(UnsupportedReasonV1::UnsupportedSide {
                    side: input.side_affected,
                });
                None
            } else if input.value == 0 && input.value_min == 0 && input.value_max == 0 {
                Some(SupportedEffectV1::StopOpponentAbility)
            } else {
                reasons.insert(UnsupportedReasonV1::NonZeroControlValues);
                None
            }
        }
        (AttributeActionV1::None, SpecialActionV1::StopBonus, _)
            if input.attribute_affected == AttributeAffectedV1::None =>
        {
            if input.side_affected != AffectedSideV1::Player {
                reasons.insert(UnsupportedReasonV1::UnsupportedSide {
                    side: input.side_affected,
                });
                None
            } else if input.value == 0 && input.value_min == 0 && input.value_max == 0 {
                Some(SupportedEffectV1::StopOpponentBonus)
            } else {
                reasons.insert(UnsupportedReasonV1::NonZeroControlValues);
                None
            }
        }
        // Protection names the side it defends, always the owner's own. It carries no
        // magnitude of its own: a non-zero value would be a different, unreviewed shape.
        (AttributeActionV1::Protect, SpecialActionV1::None, Some(stat)) => {
            if input.side_affected != AffectedSideV1::Player {
                reasons.insert(UnsupportedReasonV1::UnsupportedSide {
                    side: input.side_affected,
                });
                None
            } else if input.value == 0 && input.value_min == 0 && input.value_max == 0 {
                Some(SupportedEffectV1::ProtectOwnCombatStat { stat })
            } else {
                reasons.insert(UnsupportedReasonV1::NonZeroControlValues);
                None
            }
        }
        (AttributeActionV1::None, SpecialActionV1::ProtectAbility, _)
            if input.attribute_affected == AttributeAffectedV1::None =>
        {
            if input.side_affected != AffectedSideV1::Player {
                reasons.insert(UnsupportedReasonV1::UnsupportedSide {
                    side: input.side_affected,
                });
                None
            } else if input.value == 0 && input.value_min == 0 && input.value_max == 0 {
                Some(SupportedEffectV1::ProtectOwnAbility)
            } else {
                reasons.insert(UnsupportedReasonV1::NonZeroControlValues);
                None
            }
        }
        (AttributeActionV1::None, SpecialActionV1::ProtectBonus, _)
            if input.attribute_affected == AttributeAffectedV1::None =>
        {
            if input.side_affected != AffectedSideV1::Player {
                reasons.insert(UnsupportedReasonV1::UnsupportedSide {
                    side: input.side_affected,
                });
                None
            } else if input.value == 0 && input.value_min == 0 && input.value_max == 0 {
                Some(SupportedEffectV1::ProtectOwnBonus)
            } else {
                reasons.insert(UnsupportedReasonV1::NonZeroControlValues);
                None
            }
        }
        (AttributeActionV1::StopModifier, SpecialActionV1::None, Some(stat)) => {
            if input.side_affected == AffectedSideV1::Opponent {
                Some(SupportedEffectV1::CancelOpponentCombatStatModifiers { stat })
            } else {
                reasons.insert(UnsupportedReasonV1::UnsupportedSide {
                    side: input.side_affected,
                });
                None
            }
        }
        _ => {
            if input.attribute_action != AttributeActionV1::None {
                reasons.insert(UnsupportedReasonV1::AttributeAction {
                    action: input.attribute_action,
                });
            }
            if input.special_action != SpecialActionV1::None {
                reasons.insert(UnsupportedReasonV1::SpecialAction {
                    action: input.special_action,
                });
            }
            if CombatStatV1::from_attribute(input.attribute_affected).is_none()
                && input.attribute_affected != AttributeAffectedV1::None
            {
                reasons.insert(UnsupportedReasonV1::Attribute {
                    attribute: input.attribute_affected,
                });
            }
            None
        }
    };

    if input.is_support && !matches!(candidate, Some(SupportedEffectV1::ModifyCombatStat { .. })) {
        reasons.insert(UnsupportedReasonV1::LinkedMagnitude {
            link: LinkedMagnitudeV1::Support,
        });
    }

    if let Some(effect) = candidate {
        if let Some(context) = unreviewed_description_context(description, effect) {
            reasons.insert(UnsupportedReasonV1::DescriptionContext { context });
        }
    }

    match candidate {
        Some(effect) if reasons.is_empty() => CompiledEffectV1::Supported(effect),
        _ => {
            if reasons.is_empty() {
                reasons.insert(UnsupportedReasonV1::UnrecognizedShape);
            }
            CompiledEffectV1::Unsupported(reasons.into_iter().collect())
        }
    }
}

fn explicit_description_context(description: &str) -> Option<DescriptionContextV1> {
    if description.starts_with("Team:") {
        Some(DescriptionContextV1::Team)
    } else if description.starts_with("Day:") {
        Some(DescriptionContextV1::Day)
    } else if description.starts_with("Night:") {
        Some(DescriptionContextV1::Night)
    } else if description.contains("Cards") {
        Some(DescriptionContextV1::Cards)
    } else {
        None
    }
}

fn unreviewed_description_context(
    description: &str,
    effect: SupportedEffectV1,
) -> Option<DescriptionContextV1> {
    if explicit_description_context(description).is_some() {
        return None;
    }

    let reviewed = match effect {
        SupportedEffectV1::ModifyCombatStat {
            side,
            stat,
            operation,
            value,
            minimum,
            maximum,
            multiplier,
        } => reviewed_stat_description(
            description,
            side,
            stat,
            operation,
            value,
            minimum,
            maximum,
            multiplier,
        ),
        SupportedEffectV1::StopOpponentAbility => description == "Stop Opp. Ability",
        SupportedEffectV1::StopOpponentBonus => description == "Stop Opp. Bonus",
        SupportedEffectV1::CancelOpponentCombatStatModifiers { stat } => {
            let stat = match stat {
                CombatStatV1::Attack => "Attack",
                CombatStatV1::Damage => "Damage",
                CombatStatV1::Power => "Power",
                CombatStatV1::PowerAndDamage => "Power And Damage",
            };
            description == format!("Cancel Opp. {stat} Modif.")
        }
        // Only the Power And Damage pairing has observed rounds. `Protection: Power`,
        // `Protection : Damage` and `Protection: Attack` keep their own grammars, and the
        // site's spaced punctuation is not accepted for any of them.
        SupportedEffectV1::ProtectOwnCombatStat { stat } => {
            stat == CombatStatV1::PowerAndDamage && description == "Protection: Power And Damage"
        }
        SupportedEffectV1::ProtectOwnAbility => description == "Protection: Ability",
        SupportedEffectV1::ProtectOwnBonus => description == "Protection: Bonus",
    };
    (!reviewed).then_some(DescriptionContextV1::OtherUnreviewedGrammar)
}

#[allow(clippy::too_many_arguments)]
fn reviewed_stat_description(
    description: &str,
    side: AffectedSideV1,
    stat: CombatStatV1,
    operation: StatOperationV1,
    value: u16,
    minimum: Option<u16>,
    maximum: Option<u16>,
    multiplier: MagnitudeMultiplierV1,
) -> bool {
    let stat = match stat {
        CombatStatV1::Attack => "Attack",
        CombatStatV1::Damage => "Damage",
        CombatStatV1::Power => "Power",
        CombatStatV1::PowerAndDamage => "Power And Damage",
    };
    let prefix = match multiplier {
        MagnitudeMultiplierV1::Fixed => "",
        MagnitudeMultiplierV1::Support => "Support: ",
        MagnitudeMultiplierV1::Growth => "Growth: ",
        MagnitudeMultiplierV1::Degrowth => "Degrowth: ",
        MagnitudeMultiplierV1::OpponentStars => "Equalizer: ",
    };
    let expected = match (side, operation) {
        (AffectedSideV1::Player, StatOperationV1::Increase) => {
            let maximum = maximum
                .map(|maximum| format!(", Max. {maximum}"))
                .unwrap_or_default();
            format!("{prefix}{stat} +{value}{maximum}")
        }
        (AffectedSideV1::Opponent, StatOperationV1::Decrease) => {
            let minimum = minimum.unwrap_or(0);
            format!("{prefix}-{value} Opp {stat}, Min {minimum}")
        }
        _ => return false,
    };
    description == expected
}

fn common_unsupported_reasons(input: &StructuredEffectV1) -> BTreeSet<UnsupportedReasonV1> {
    let mut reasons = BTreeSet::new();
    if input.position_requirement != PositionRequirementV1::Both {
        reasons.insert(UnsupportedReasonV1::Conditional {
            feature: ConditionalFeatureV1::Position,
        });
    }
    if input.previous_round_requirement != PreviousRoundRequirementV1::Any {
        reasons.insert(UnsupportedReasonV1::Conditional {
            feature: ConditionalFeatureV1::PreviousRound,
        });
    }
    if input.current_round_requirement != CurrentRoundRequirementV1::Any {
        reasons.insert(UnsupportedReasonV1::Conditional {
            feature: ConditionalFeatureV1::CurrentRound,
        });
    }
    if input.index_requirement != IndexRequirementV1::Any {
        reasons.insert(UnsupportedReasonV1::Conditional {
            feature: ConditionalFeatureV1::Index,
        });
    }
    if !input.clan_requirement.is_empty() {
        reasons.insert(UnsupportedReasonV1::Conditional {
            feature: ConditionalFeatureV1::Clan,
        });
    }
    if !input.opponent_clan_requirement.is_empty() {
        reasons.insert(UnsupportedReasonV1::Conditional {
            feature: ConditionalFeatureV1::OpponentClan,
        });
    }
    if !input.previous_clan_requirement.is_empty() {
        reasons.insert(UnsupportedReasonV1::Conditional {
            feature: ConditionalFeatureV1::PreviousClan,
        });
    }
    if input.bet_pillz_link != BetPillzLinkV1::No {
        reasons.insert(UnsupportedReasonV1::Conditional {
            feature: ConditionalFeatureV1::BetPillz,
        });
    }
    if input.value_condition != 0 {
        reasons.insert(UnsupportedReasonV1::Conditional {
            feature: ConditionalFeatureV1::ValueCondition,
        });
    }
    if input.is_inverted {
        reasons.insert(UnsupportedReasonV1::Inverted);
    }
    if input.is_anti_support {
        reasons.insert(UnsupportedReasonV1::LinkedMagnitude {
            link: LinkedMagnitudeV1::AntiSupport,
        });
    }
    if input.is_overdrive {
        reasons.insert(UnsupportedReasonV1::LinkedMagnitude {
            link: LinkedMagnitudeV1::Overdrive,
        });
    }
    if input.is_divide {
        reasons.insert(UnsupportedReasonV1::LinkedMagnitude {
            link: LinkedMagnitudeV1::Divide,
        });
    }
    if input.is_life_linked {
        reasons.insert(UnsupportedReasonV1::LinkedMagnitude {
            link: LinkedMagnitudeV1::Life,
        });
    }
    if input.is_pillz_linked {
        reasons.insert(UnsupportedReasonV1::LinkedMagnitude {
            link: LinkedMagnitudeV1::Pillz,
        });
    }
    if input.is_lost_life_linked {
        reasons.insert(UnsupportedReasonV1::LinkedMagnitude {
            link: LinkedMagnitudeV1::LostLife,
        });
    }
    if input.is_lost_pillz_linked {
        reasons.insert(UnsupportedReasonV1::LinkedMagnitude {
            link: LinkedMagnitudeV1::LostPillz,
        });
    }
    if input.is_opponent_stars_linked {
        reasons.insert(UnsupportedReasonV1::LinkedMagnitude {
            link: LinkedMagnitudeV1::OpponentStars,
        });
    }
    if input.is_clanmates_count_linked {
        reasons.insert(UnsupportedReasonV1::LinkedMagnitude {
            link: LinkedMagnitudeV1::ClanmatesCount,
        });
    }
    if input.is_anti_clanmates_count_linked {
        reasons.insert(UnsupportedReasonV1::LinkedMagnitude {
            link: LinkedMagnitudeV1::AntiClanmatesCount,
        });
    }
    if input.is_permanent {
        reasons.insert(UnsupportedReasonV1::Permanent);
    }
    if input.is_immediate_permanent {
        reasons.insert(UnsupportedReasonV1::ImmediatePermanent);
    }
    reasons
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    bytes.iter().fold(OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::CardCatalog;
    use crate::replay::load_corpus;
    use serde_json::{json, Value};

    fn root_path(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(path)
    }

    fn dictionary_path() -> PathBuf {
        root_path("captures/abilities.json")
    }

    fn base_entry(id: u32) -> Value {
        json!({
            "id": id,
            "unlockLevel": 0,
            "description": "Power +2",
            "longDescription": "test",
            "abilityData": {
                "value": 2,
                "valueMin": 0,
                "valueMax": 0,
                "valueCondition": 0,
                "positionRequirement": "both",
                "previousRoundRequirement": "any",
                "currentRoundRequirement": "any",
                "indexRequirement": "any",
                "clanRequirement": "",
                "oppClanRequirement": "",
                "previousClanRequirement": "",
                "betPillzLink": "no",
                "sideAffected": "player",
                "attributeAffected": "pwr",
                "attributeAction": "increase",
                "specialAction": "none",
                "isInverted": false,
                "isSupport": false,
                "isAntiSupport": false,
                "isOverdrive": false,
                "isDivide": false,
                "isLifeLinked": false,
                "isPillzLinked": false,
                "isLostLifeLinked": false,
                "isLostPillzLinked": false,
                "isOppStarsLinked": false,
                "isClanmatesCountLinked": false,
                "isAntiClanmatesCountLinked": false,
                "isPermanent": false,
                "isImmediatePermanent": false
            }
        })
    }

    fn single_entry_json(entry: Value) -> Vec<u8> {
        let id = entry["id"].as_u64().unwrap();
        serde_json::to_vec(&json!({ id.to_string(): entry })).unwrap()
    }

    #[test]
    fn loads_the_full_dictionary_deterministically() {
        let first = EffectRegistryV1::load(dictionary_path()).unwrap();
        let second = EffectRegistryV1::load(dictionary_path()).unwrap();
        let ids: Vec<_> = first.iter().map(|(id, _)| id).collect();

        assert!(first.len() >= 996);
        assert_eq!(first.schema_version(), EFFECT_REGISTRY_SCHEMA_VERSION);
        assert_eq!(
            first.source_fingerprint_fnv1a64(),
            second.source_fingerprint_fnv1a64()
        );
        assert!(ids.windows(2).all(|ids| ids[0] < ids[1]));
        assert!(first.iter().all(|(id, definition)| id == definition.id()));
    }

    #[test]
    fn exposes_every_observed_string_domain() {
        assert_eq!(
            PositionRequirementV1::DOMAIN,
            ["attacker", "both", "defender"]
        );
        assert_eq!(PreviousRoundRequirementV1::DOMAIN, ["any", "lose", "win"]);
        assert_eq!(
            CurrentRoundRequirementV1::DOMAIN,
            ["any", "lose", "perfect", "sureshot", "win"]
        );
        assert_eq!(IndexRequirementV1::DOMAIN, ["any", "asymmetry", "symmetry"]);
        assert_eq!(BetPillzLinkV1::DOMAIN, ["less", "more", "no"]);
        assert_eq!(AffectedSideV1::DOMAIN, ["both", "opponent", "player"]);
        assert_eq!(
            AttributeAffectedV1::DOMAIN,
            [
                "atk",
                "dmg",
                "life",
                "life&pillz",
                "none",
                "pillz",
                "pwr",
                "pwr&atk",
                "pwr&dmg"
            ]
        );
        assert_eq!(
            AttributeActionV1::DOMAIN,
            [
                "copy",
                "decrease",
                "increase",
                "none",
                "protect",
                "simplify",
                "stop_modif"
            ]
        );
        assert_eq!(SpecialActionV1::DOMAIN.len(), 16);
    }

    #[test]
    fn rejects_map_mismatch_exact_duplicate_and_numeric_alias() {
        let mismatch = serde_json::to_vec(&json!({"7": base_entry(8)})).unwrap();
        assert!(matches!(
            EffectRegistryV1::from_reader(mismatch.as_slice()),
            Err(EffectRegistryError::EmbeddedIdMismatch {
                map_key_id: 7,
                embedded_id: 8,
                ..
            })
        ));

        let entry = String::from_utf8(single_entry_json(base_entry(1))).unwrap();
        let body = entry.strip_prefix('{').unwrap().strip_suffix('}').unwrap();
        let duplicate = format!("{{{body},{body}}}");
        assert!(matches!(
            EffectRegistryV1::from_reader(duplicate.as_bytes()),
            Err(EffectRegistryError::DuplicateMapKey { ref key }) if key == "1"
        ));

        let first = serde_json::to_string(&base_entry(1)).unwrap();
        let alias = format!(r#"{{"1":{first},"01":{first}}}"#);
        assert!(matches!(
            EffectRegistryV1::from_reader(alias.as_bytes()),
            Err(EffectRegistryError::NumericKeyAlias { effect_id: 1, .. })
        ));
    }

    #[test]
    fn rejects_unknown_enum_and_malformed_combinations_contextually() {
        let mut unknown = base_entry(77);
        unknown["abilityData"]["attributeAction"] = json!("mystery");
        assert!(matches!(
            EffectRegistryV1::from_reader(single_entry_json(unknown).as_slice()),
            Err(EffectRegistryError::UnknownEnumValue {
                effect_id: 77,
                field: "attributeAction",
                ref value,
                ..
            }) if value == "mystery"
        ));

        let mut malformed = base_entry(78);
        malformed["abilityData"]["isSupport"] = json!(true);
        malformed["abilityData"]["isAntiSupport"] = json!(true);
        assert!(matches!(
            EffectRegistryV1::from_reader(single_entry_json(malformed).as_slice()),
            Err(EffectRegistryError::InvalidCombination {
                effect_id: 78,
                field: "isSupport/isAntiSupport",
                ..
            })
        ));

        let mut unknown_field = base_entry(79);
        unknown_field["abilityData"]["futureSemanticField"] = json!(true);
        assert!(matches!(
            EffectRegistryV1::from_reader(single_entry_json(unknown_field).as_slice()),
            Err(EffectRegistryError::Parse(_))
        ));

        let mut negative = base_entry(80);
        negative["abilityData"]["value"] = json!(-1);
        assert!(matches!(
            EffectRegistryV1::from_reader(single_entry_json(negative).as_slice()),
            Err(EffectRegistryError::Parse(_))
        ));
    }

    #[test]
    fn compiles_only_the_first_evidence_backed_shapes() {
        let registry = EffectRegistryV1::load(dictionary_path()).unwrap();

        assert_eq!(
            registry.get(6).unwrap().compiled().supported(),
            Some(SupportedEffectV1::ModifyCombatStat {
                side: AffectedSideV1::Opponent,
                stat: CombatStatV1::Attack,
                operation: StatOperationV1::Decrease,
                value: 12,
                minimum: Some(8),
                maximum: None,
                multiplier: MagnitudeMultiplierV1::Fixed,
            })
        );
        assert!(matches!(
            registry.get(266).unwrap().compiled().supported(),
            Some(SupportedEffectV1::ModifyCombatStat {
                stat: CombatStatV1::Attack,
                multiplier: MagnitudeMultiplierV1::Support,
                value: 3,
                ..
            })
        ));
        for (id, stat, operation, value) in [
            (43, CombatStatV1::Power, StatOperationV1::Increase, 2),
            (38, CombatStatV1::Damage, StatOperationV1::Increase, 2),
            (156, CombatStatV1::Power, StatOperationV1::Decrease, 2),
            (
                4618,
                CombatStatV1::PowerAndDamage,
                StatOperationV1::Increase,
                2,
            ),
        ] {
            assert!(matches!(
                registry.get(id).unwrap().compiled().supported(),
                Some(SupportedEffectV1::ModifyCombatStat {
                    stat: actual_stat,
                    operation: actual_operation,
                    value: actual_value,
                    ..
                }) if actual_stat == stat && actual_operation == operation && actual_value == value
            ));
        }
        assert_eq!(
            registry.get(130).unwrap().compiled().supported(),
            Some(SupportedEffectV1::StopOpponentBonus)
        );
        assert_eq!(
            registry.get(41).unwrap().compiled().supported(),
            Some(SupportedEffectV1::StopOpponentAbility)
        );
        assert_eq!(
            registry.get(1163).unwrap().compiled().supported(),
            Some(SupportedEffectV1::CancelOpponentCombatStatModifiers {
                stat: CombatStatV1::Attack
            })
        );
        assert!(matches!(
            registry.get(2969).unwrap().compiled().supported(),
            Some(SupportedEffectV1::ModifyCombatStat {
                stat: CombatStatV1::Power,
                maximum: Some(8),
                ..
            })
        ));
        assert!(matches!(
            registry.get(2535).unwrap().compiled().supported(),
            Some(SupportedEffectV1::ModifyCombatStat {
                operation: StatOperationV1::Decrease,
                minimum: Some(0),
                multiplier: MagnitudeMultiplierV1::Support,
                ..
            })
        ));
        assert!(matches!(
            registry.get(877).unwrap().compiled(),
            CompiledEffectV1::Unsupported(reasons)
                if reasons.contains(&UnsupportedReasonV1::NonZeroControlValues)
        ));
        assert!(matches!(
            registry.get(425).unwrap().compiled(),
            CompiledEffectV1::Unsupported(reasons)
                if reasons.contains(&UnsupportedReasonV1::Conditional {
                    feature: ConditionalFeatureV1::Position,
                })
        ));
        for (id, link) in [
            (1241, LinkedMagnitudeV1::Overdrive),
            (1580, LinkedMagnitudeV1::Divide),
        ] {
            assert!(registry
                .get(id)
                .unwrap()
                .compiled()
                .unsupported_reasons()
                .contains(&UnsupportedReasonV1::LinkedMagnitude { link }));
        }
    }

    #[test]
    fn protection_compiles_only_the_three_reviewed_printed_grammars() {
        let registry = EffectRegistryV1::load(dictionary_path()).unwrap();

        // Like generic Victory Life, Protection is admitted by exact printed text and
        // structured shape, not by an id list: the registry carries many structurally
        // identical definitions of each.
        for id in [759, 880, 1355, 1464, 1793, 2295, 3232, 3550, 5761] {
            assert_eq!(
                registry.get(id).unwrap().compiled().supported(),
                Some(SupportedEffectV1::ProtectOwnCombatStat {
                    stat: CombatStatV1::PowerAndDamage
                }),
                "effect {id}"
            );
        }
        assert_eq!(
            registry.get(461).unwrap().compiled().supported(),
            Some(SupportedEffectV1::ProtectOwnAbility)
        );
        for id in [481, 1132, 1515, 1554, 2860, 4098, 4983, 5498] {
            assert_eq!(
                registry.get(id).unwrap().compiled().supported(),
                Some(SupportedEffectV1::ProtectOwnBonus),
                "effect {id}"
            );
        }

        // Every other Protection grammar has no reviewed round behind it and stays out,
        // including the site's spaced `Protection : Damage` and the clan-conditional one.
        for id in [728, 940, 956, 1142, 1311, 2294, 2376, 2981, 4660, 5708] {
            assert_eq!(
                registry.get(id).unwrap().compiled().supported(),
                None,
                "effect {id}"
            );
        }
    }

    #[test]
    fn description_only_context_never_inherits_a_structured_classification() {
        let registry = EffectRegistryV1::load(dictionary_path()).unwrap();
        let team = registry.get(4237).unwrap();
        let ordinary = registry.get(5229).unwrap();
        assert_eq!(team.structured_input(), ordinary.structured_input());
        assert!(matches!(
            ordinary.compiled().supported(),
            Some(SupportedEffectV1::ModifyCombatStat {
                stat: CombatStatV1::Attack,
                value: 7,
                ..
            })
        ));
        assert!(team.compiled().unsupported_reasons().contains(
            &UnsupportedReasonV1::DescriptionContext {
                context: DescriptionContextV1::Team
            }
        ));

        let night = registry.get(5564).unwrap();
        let ordinary_stop = registry.get(41).unwrap();
        assert_eq!(night.structured_input(), ordinary_stop.structured_input());
        assert!(night.compiled().unsupported_reasons().contains(
            &UnsupportedReasonV1::DescriptionContext {
                context: DescriptionContextV1::Night
            }
        ));
    }

    #[test]
    fn support_and_target_exceptions_cannot_leak_into_stop_bonus() {
        let mut entry = base_entry(90);
        entry["description"] = json!("Stop Opp. Bonus");
        entry["abilityData"]["value"] = json!(0);
        entry["abilityData"]["attributeAffected"] = json!("none");
        entry["abilityData"]["attributeAction"] = json!("none");
        entry["abilityData"]["specialAction"] = json!("stop_bonus");
        entry["abilityData"]["isSupport"] = json!(true);
        let registry = EffectRegistryV1::from_reader(single_entry_json(entry).as_slice()).unwrap();
        assert!(registry
            .get(90)
            .unwrap()
            .compiled()
            .unsupported_reasons()
            .contains(&UnsupportedReasonV1::LinkedMagnitude {
                link: LinkedMagnitudeV1::Support
            }));

        let mut entry = base_entry(91);
        entry["description"] = json!("Stop Opp. Bonus");
        entry["abilityData"]["value"] = json!(0);
        entry["abilityData"]["attributeAffected"] = json!("none");
        entry["abilityData"]["attributeAction"] = json!("none");
        entry["abilityData"]["specialAction"] = json!("stop_bonus");
        entry["abilityData"]["sideAffected"] = json!("opponent");
        let registry = EffectRegistryV1::from_reader(single_entry_json(entry).as_slice()).unwrap();
        assert!(registry
            .get(91)
            .unwrap()
            .compiled()
            .unsupported_reasons()
            .contains(&UnsupportedReasonV1::UnsupportedSide {
                side: AffectedSideV1::Opponent
            }));

        for (offset, attribute) in AttributeAffectedV1::DOMAIN
            .iter()
            .filter(|attribute| **attribute != "none")
            .enumerate()
        {
            let id = 100 + u32::try_from(offset).unwrap();
            let mut entry = base_entry(id);
            entry["description"] = json!("Stop Opp. Bonus");
            entry["abilityData"]["value"] = json!(0);
            entry["abilityData"]["attributeAffected"] = json!(attribute);
            entry["abilityData"]["attributeAction"] = json!("none");
            entry["abilityData"]["specialAction"] = json!("stop_bonus");
            let registry =
                EffectRegistryV1::from_reader(single_entry_json(entry).as_slice()).unwrap();
            assert!(
                registry.get(id).unwrap().compiled().supported().is_none(),
                "Stop Bonus with attributeAffected={attribute} must fail closed"
            );
        }
    }

    #[test]
    fn capture_lookup_is_strict_and_catalog_fallback_exposes_aliases() {
        let registry = EffectRegistryV1::load(dictionary_path()).unwrap();
        assert_eq!(
            registry
                .lookup_capture(266, "Support: Attack +3")
                .unwrap()
                .id(),
            266
        );
        assert!(matches!(
            registry.lookup_capture(266, "Attack +3"),
            Err(EffectLookupError::DescriptionMismatch { id: 266, .. })
        ));

        let rescue = registry.lookup_description("Support: Attack +3").unwrap();
        assert_eq!(rescue.definition().id(), 266);
        assert_eq!(rescue.alias_ids(), [266, 546, 5841]);

        let catalog = CardCatalog::load(root_path("data/data.json")).unwrap();
        assert!(catalog
            .iter()
            .any(|(_, card)| card.bonus_id == 39 && card.bonus == "Support: Attack +3"));
        assert!(matches!(
            registry.lookup_description("Stop Opp. Ability"),
            Err(EffectLookupError::AmbiguousDescription { ref ids, .. }) if ids.contains(&877)
        ));

        let one = serde_json::to_string(&base_entry(1)).unwrap();
        let two = serde_json::to_string(&base_entry(2)).unwrap();
        let reversed_source = format!(r#"{{"2":{two},"1":{one}}}"#);
        let reversed = EffectRegistryV1::from_reader(reversed_source.as_bytes()).unwrap();
        let match_ = reversed.lookup_description("Power +2").unwrap();
        assert_eq!(match_.definition().id(), 1);
        assert_eq!(match_.alias_ids(), [1, 2]);
    }

    #[test]
    fn empty_dictionary_is_fatal() {
        assert!(matches!(
            EffectRegistryV1::from_reader(b"{}".as_slice()),
            Err(EffectRegistryError::EmptyRegistry)
        ));
    }

    #[test]
    fn unsupported_effects_remain_explicit_at_the_registry_boundary() {
        let registry = EffectRegistryV1::load(dictionary_path()).unwrap();
        assert!(matches!(
            registry.get(877).unwrap().compiled(),
            CompiledEffectV1::Unsupported(_)
        ));
    }

    #[test]
    fn source_fingerprint_is_stable_and_sensitive_to_source_bytes() {
        let source = single_entry_json(base_entry(1));
        let first = EffectRegistryV1::from_reader(source.as_slice()).unwrap();
        let second = EffectRegistryV1::from_reader(source.as_slice()).unwrap();
        let mut changed = source.clone();
        changed.push(b' ');
        let changed = EffectRegistryV1::from_reader(changed.as_slice()).unwrap();

        assert_eq!(
            first.source_fingerprint_fnv1a64(),
            second.source_fingerprint_fnv1a64()
        );
        assert_ne!(
            first.source_fingerprint_fnv1a64(),
            changed.source_fingerprint_fnv1a64()
        );
        assert!(first
            .source_fingerprint_fnv1a64()
            .to_string()
            .starts_with("fnv1a64:"));
    }

    #[test]
    fn resolves_every_modifier_reference_in_the_growing_replay_corpus() {
        let registry = EffectRegistryV1::load(dictionary_path()).unwrap();
        let corpus = load_corpus(root_path("captures/games"), root_path("data/data.json")).unwrap();
        assert!(corpus.errors.is_empty(), "{:#?}", corpus.errors);
        assert!(corpus.ready.len() >= 322);
        assert!(
            corpus
                .ready
                .iter()
                .map(|replay| replay.rounds.len())
                .sum::<usize>()
                >= 1_102
        );

        let mut references = 0;
        for replay in &corpus.ready {
            for player in &replay.players {
                for card in &player.hand {
                    for modifier in [&card.source_ability, &card.source_bonus]
                        .into_iter()
                        .filter_map(Option::as_ref)
                    {
                        registry
                            .lookup_capture(modifier.id, &modifier.description)
                            .unwrap_or_else(|error| {
                                panic!(
                                    "battle {} card {:?} modifier {} failed lookup: {error}",
                                    replay.metadata.battle_id, card.key, modifier.id
                                )
                            });
                        references += 1;
                    }
                }
            }
        }
        assert!(references >= 4_324, "only {references} modifier references");
    }
}
