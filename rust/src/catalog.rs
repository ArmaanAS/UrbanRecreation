//! Canonical card data keyed by the identity used in captured games.
//!
//! This module deliberately does not construct the legacy engine's [`crate::card::Card`]
//! type. It is the data boundary that future replay and parity work can build on without
//! changing existing game rules.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// The stable identity of one playable card level.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CardKey {
    pub id: u32,
    pub level: u8,
}

impl CardKey {
    pub const fn new(id: u32, level: u8) -> Self {
        Self { id, level }
    }
}

/// One row from the canonical `data/data.json` card corpus.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct CanonicalCard {
    pub id: u32,
    pub name: String,
    pub clan_id: u32,
    pub clan_name: String,
    pub level: u8,
    pub level_min: u8,
    pub level_max: u8,
    pub power: u8,
    pub damage: u8,
    pub rarity: String,
    pub ability_id: u32,
    pub ability: String,
    pub ability_unlock_level: u8,
    pub bonus: String,
    pub bonus_id: u32,
    pub release_date: i64,
    #[serde(default)]
    pub night_ability: Option<String>,
    #[serde(default)]
    pub night_bonus: Option<String>,
}

/// One internally consistent clan definition derived from the canonical card rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalClan {
    pub id: u32,
    pub name: String,
    pub bonus_id: u32,
    pub bonus: String,
    pub night_bonus: Option<String>,
}

impl CanonicalCard {
    pub const fn key(&self) -> CardKey {
        CardKey::new(self.id, self.level)
    }
}

/// A deterministic index of canonical card rows by card id and level.
#[derive(Clone, Debug, Default)]
pub struct CardCatalog {
    by_key: BTreeMap<CardKey, CanonicalCard>,
    by_clan_id: BTreeMap<u32, CanonicalClan>,
}

/// Canonical card data after the same reviewed live overrides used by the TypeScript
/// runtime have been validated and applied.
///
/// Keeping this as a distinct type prevents a solver-facing constructor from silently
/// accepting raw catalog rows that the reference engine replaces at load time.
#[derive(Clone, Debug)]
pub struct EffectiveCardCatalog {
    catalog: CardCatalog,
    validated_override_count: usize,
    source_fingerprint_fnv1a64: EffectiveCatalogSourceFingerprintFnv1a64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EffectiveCardDefinitionV1 {
    pub power: u8,
    pub damage: u8,
    pub ability_id: u32,
    pub ability: String,
}

/// FNV-1a over the exact canonical-catalog and override-contract bytes supplied to the
/// effective catalog loader. This is a deterministic change detector, not a cryptographic
/// content hash.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct EffectiveCatalogSourceFingerprintFnv1a64(u64);

impl EffectiveCatalogSourceFingerprintFnv1a64 {
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for EffectiveCatalogSourceFingerprintFnv1a64 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "fnv1a64:{:016x}", self.0)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BattleCardOverrideV1 {
    id: u32,
    name: String,
    level: u8,
    from: EffectiveCardDefinitionV1,
    to: EffectiveCardDefinitionV1,
    #[serde(rename = "sourceBattle")]
    source_battle: u64,
}

impl CardCatalog {
    /// Loads a catalog from an explicit path.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, CatalogError> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|source| CatalogError::Open {
            path: path.to_path_buf(),
            source,
        })?;

        Self::from_reader(file)
    }

    /// Loads a catalog from JSON containing an array of canonical card rows.
    pub fn from_reader(reader: impl Read) -> Result<Self, CatalogError> {
        let rows: Vec<CanonicalCard> =
            serde_json::from_reader(reader).map_err(CatalogError::Parse)?;
        let mut by_key = BTreeMap::new();
        let mut by_clan_id: BTreeMap<u32, CanonicalClan> = BTreeMap::new();

        for row in rows {
            let key = row.key();
            let clan = CanonicalClan {
                id: row.clan_id,
                name: row.clan_name.clone(),
                bonus_id: row.bonus_id,
                bonus: row.bonus.clone(),
                night_bonus: row.night_bonus.clone(),
            };
            if let Some(existing) = by_clan_id.get(&row.clan_id) {
                if existing != &clan {
                    return Err(CatalogError::InconsistentClan {
                        key,
                        expected: existing.clone(),
                        actual: clan,
                    });
                }
            } else {
                by_clan_id.insert(row.clan_id, clan);
            }
            if by_key.insert(key, row).is_some() {
                return Err(CatalogError::DuplicateKey { key });
            }
        }

        Ok(Self { by_key, by_clan_id })
    }

    pub fn get(&self, key: CardKey) -> Option<&CanonicalCard> {
        self.by_key.get(&key)
    }

    pub fn get_clan(&self, clan_id: u32) -> Option<&CanonicalClan> {
        self.by_clan_id.get(&clan_id)
    }

    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }

    /// Iterates in ascending `(id, level)` order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (CardKey, &CanonicalCard)> {
        self.by_key.iter().map(|(&key, card)| (key, card))
    }

    /// Iterates in ascending numeric clan-id order.
    pub fn clans(&self) -> impl ExactSizeIterator<Item = (u32, &CanonicalClan)> {
        self.by_clan_id.iter().map(|(&id, clan)| (id, clan))
    }
}

impl EffectiveCardCatalog {
    /// Loads raw canonical rows and the explicit runtime override contract.
    pub fn load(
        catalog_path: impl AsRef<Path>,
        overrides_path: impl AsRef<Path>,
    ) -> Result<Self, EffectiveCatalogError> {
        let catalog_path = catalog_path.as_ref();
        let overrides_path = overrides_path.as_ref();
        let catalog = File::open(catalog_path).map_err(|source| {
            EffectiveCatalogError::Catalog(CatalogError::Open {
                path: catalog_path.to_path_buf(),
                source,
            })
        })?;
        let overrides =
            File::open(overrides_path).map_err(|source| EffectiveCatalogError::OpenOverrides {
                path: overrides_path.to_path_buf(),
                source,
            })?;
        Self::from_readers(catalog, overrides)
    }

    /// Builds an effective catalog from explicit readers. This is primarily useful for
    /// deterministic tests and callers that already own the source bytes.
    pub fn from_readers(
        mut catalog_reader: impl Read,
        mut overrides_reader: impl Read,
    ) -> Result<Self, EffectiveCatalogError> {
        let mut catalog_bytes = Vec::new();
        catalog_reader
            .read_to_end(&mut catalog_bytes)
            .map_err(EffectiveCatalogError::ReadCatalog)?;
        let mut overrides_bytes = Vec::new();
        overrides_reader
            .read_to_end(&mut overrides_bytes)
            .map_err(EffectiveCatalogError::ReadOverrides)?;
        let source_fingerprint_fnv1a64 =
            effective_catalog_source_fingerprint(&catalog_bytes, &overrides_bytes);
        let catalog = CardCatalog::from_reader(catalog_bytes.as_slice())
            .map_err(EffectiveCatalogError::Catalog)?;
        let overrides: Vec<BattleCardOverrideV1> = serde_json::from_slice(&overrides_bytes)
            .map_err(EffectiveCatalogError::ParseOverrides)?;
        Self::from_catalog_and_overrides(catalog, overrides, source_fingerprint_fnv1a64)
    }

    fn from_catalog_and_overrides(
        mut catalog: CardCatalog,
        overrides: Vec<BattleCardOverrideV1>,
        source_fingerprint_fnv1a64: EffectiveCatalogSourceFingerprintFnv1a64,
    ) -> Result<Self, EffectiveCatalogError> {
        let mut keys = BTreeSet::new();
        for override_ in &overrides {
            let key = CardKey::new(override_.id, override_.level);
            if !keys.insert(key) {
                return Err(EffectiveCatalogError::DuplicateOverride { key });
            }
            let card =
                catalog
                    .by_key
                    .get_mut(&key)
                    .ok_or(EffectiveCatalogError::MissingOverrideCard {
                        key,
                        source_battle: override_.source_battle,
                    })?;
            if card.name != override_.name {
                return Err(EffectiveCatalogError::OverrideNameMismatch {
                    key,
                    source_battle: override_.source_battle,
                    expected: override_.name.clone(),
                    actual: card.name.clone(),
                });
            }
            let actual = effective_definition(card);
            if actual == override_.to {
                continue;
            }
            if actual != override_.from {
                return Err(EffectiveCatalogError::UnexpectedOverrideDefinition {
                    key,
                    source_battle: override_.source_battle,
                    actual,
                    expected_from: override_.from.clone(),
                    expected_to: override_.to.clone(),
                });
            }
            card.power = override_.to.power;
            card.damage = override_.to.damage;
            card.ability_id = override_.to.ability_id;
            card.ability.clone_from(&override_.to.ability);
        }
        Ok(Self {
            catalog,
            validated_override_count: overrides.len(),
            source_fingerprint_fnv1a64,
        })
    }

    pub fn get(&self, key: CardKey) -> Option<&CanonicalCard> {
        self.catalog.get(key)
    }

    pub fn get_clan(&self, clan_id: u32) -> Option<&CanonicalClan> {
        self.catalog.get_clan(clan_id)
    }

    pub fn len(&self) -> usize {
        self.catalog.len()
    }

    pub fn is_empty(&self) -> bool {
        self.catalog.is_empty()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (CardKey, &CanonicalCard)> {
        self.catalog.iter()
    }

    pub fn clans(&self) -> impl ExactSizeIterator<Item = (u32, &CanonicalClan)> {
        self.catalog.clans()
    }

    pub const fn validated_override_count(&self) -> usize {
        self.validated_override_count
    }

    pub const fn source_fingerprint_fnv1a64(&self) -> EffectiveCatalogSourceFingerprintFnv1a64 {
        self.source_fingerprint_fnv1a64
    }

    pub const fn as_catalog(&self) -> &CardCatalog {
        &self.catalog
    }
}

fn effective_definition(card: &CanonicalCard) -> EffectiveCardDefinitionV1 {
    EffectiveCardDefinitionV1 {
        power: card.power,
        damage: card.damage,
        ability_id: card.ability_id,
        ability: card.ability.clone(),
    }
}

fn effective_catalog_source_fingerprint(
    catalog_bytes: &[u8],
    overrides_bytes: &[u8],
) -> EffectiveCatalogSourceFingerprintFnv1a64 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn update(hash: &mut u64, bytes: &[u8]) {
        for byte in bytes {
            *hash ^= u64::from(*byte);
            *hash = hash.wrapping_mul(PRIME);
        }
    }

    let mut hash = OFFSET_BASIS;
    update(&mut hash, b"urban-recreation-effective-catalog-v1\0");
    update(&mut hash, &(catalog_bytes.len() as u64).to_le_bytes());
    update(&mut hash, catalog_bytes);
    update(&mut hash, &(overrides_bytes.len() as u64).to_le_bytes());
    update(&mut hash, overrides_bytes);
    EffectiveCatalogSourceFingerprintFnv1a64(hash)
}

#[derive(Debug)]
pub enum CatalogError {
    Open {
        path: PathBuf,
        source: io::Error,
    },
    Parse(serde_json::Error),
    DuplicateKey {
        key: CardKey,
    },
    InconsistentClan {
        key: CardKey,
        expected: CanonicalClan,
        actual: CanonicalClan,
    },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { path, source } => {
                write!(formatter, "failed to open {}: {source}", path.display())
            }
            Self::Parse(source) => write!(formatter, "failed to parse card catalog: {source}"),
            Self::DuplicateKey { key } => {
                write!(
                    formatter,
                    "duplicate card id {} at level {}",
                    key.id, key.level
                )
            }
            Self::InconsistentClan {
                key,
                expected,
                actual,
            } => write!(
                formatter,
                "card id {} level {} has clan definition {:?}, expected {:?}",
                key.id, key.level, actual, expected
            ),
        }
    }
}

impl Error for CatalogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Open { source, .. } => Some(source),
            Self::Parse(source) => Some(source),
            Self::DuplicateKey { .. } | Self::InconsistentClan { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum EffectiveCatalogError {
    Catalog(CatalogError),
    ReadCatalog(io::Error),
    OpenOverrides {
        path: PathBuf,
        source: io::Error,
    },
    ReadOverrides(io::Error),
    ParseOverrides(serde_json::Error),
    DuplicateOverride {
        key: CardKey,
    },
    MissingOverrideCard {
        key: CardKey,
        source_battle: u64,
    },
    OverrideNameMismatch {
        key: CardKey,
        source_battle: u64,
        expected: String,
        actual: String,
    },
    UnexpectedOverrideDefinition {
        key: CardKey,
        source_battle: u64,
        actual: EffectiveCardDefinitionV1,
        expected_from: EffectiveCardDefinitionV1,
        expected_to: EffectiveCardDefinitionV1,
    },
}

impl fmt::Display for EffectiveCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalog(source) => source.fmt(formatter),
            Self::ReadCatalog(source) => {
                write!(formatter, "failed to read card catalog source: {source}")
            }
            Self::OpenOverrides { path, source } => write!(
                formatter,
                "failed to open card override contract {}: {source}",
                path.display()
            ),
            Self::ReadOverrides(source) => {
                write!(formatter, "failed to read card override contract: {source}")
            }
            Self::ParseOverrides(source) => {
                write!(formatter, "failed to parse card override contract: {source}")
            }
            Self::DuplicateOverride { key } => write!(
                formatter,
                "duplicate card override for id {} at level {}",
                key.id, key.level
            ),
            Self::MissingOverrideCard { key, source_battle } => write!(
                formatter,
                "card override from battle {source_battle} references missing id {} level {}",
                key.id, key.level
            ),
            Self::OverrideNameMismatch {
                key,
                source_battle,
                expected,
                actual,
            } => write!(
                formatter,
                "card override from battle {source_battle} for id {} level {} names {expected:?}, catalog has {actual:?}",
                key.id, key.level
            ),
            Self::UnexpectedOverrideDefinition {
                key,
                source_battle,
                actual,
                expected_from,
                expected_to,
            } => write!(
                formatter,
                "card override from battle {source_battle} for id {} level {} expected {:?} or {:?}, catalog has {:?}",
                key.id, key.level, expected_from, expected_to, actual
            ),
        }
    }
}

impl Error for EffectiveCatalogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Catalog(source) => Some(source),
            Self::ReadCatalog(source) => Some(source),
            Self::OpenOverrides { source, .. } => Some(source),
            Self::ReadOverrides(source) => Some(source),
            Self::ParseOverrides(source) => Some(source),
            Self::DuplicateOverride { .. }
            | Self::MissingOverrideCard { .. }
            | Self::OverrideNameMismatch { .. }
            | Self::UnexpectedOverrideDefinition { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CanonicalCard, CardCatalog, CardKey, CatalogError, EffectiveCardCatalog,
        EffectiveCatalogError,
    };
    use std::collections::BTreeSet;
    use std::fs::File;
    use std::path::PathBuf;

    fn canonical_data_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/data.json")
    }

    fn overrides_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/battle_card_overrides.json")
    }

    #[test]
    fn indexes_every_canonical_level_row() {
        let path = canonical_data_path();
        let rows: Vec<CanonicalCard> = serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        let catalog = CardCatalog::load(path).unwrap();

        assert_eq!(catalog.len(), rows.len());
        for row in &rows {
            assert_eq!(catalog.get(row.key()), Some(row));
        }

        let character_ids: BTreeSet<_> = rows.iter().map(|row| row.id).collect();
        assert!(character_ids.len() >= 2_496);
        assert!(catalog.len() > character_ids.len());
    }

    #[test]
    fn distinguishes_levels_of_the_same_real_card() {
        let catalog = CardCatalog::load(canonical_data_path()).unwrap();
        let level_one = catalog.get(CardKey::new(123, 1)).unwrap();
        let level_two = catalog.get(CardKey::new(123, 2)).unwrap();
        let level_three = catalog.get(CardKey::new(123, 3)).unwrap();

        assert_eq!(level_one.name, "Natrang");
        assert_eq!(level_three.name, "Natrang");
        assert_eq!((level_one.power, level_one.damage), (3, 2));
        assert_eq!((level_two.power, level_two.damage), (4, 2));
        assert_eq!((level_three.power, level_three.damage), (4, 4));
    }

    #[test]
    fn preserves_day_night_and_numeric_clan_data() {
        let catalog = CardCatalog::load(canonical_data_path()).unwrap();
        let day_night_ability = catalog.get(CardKey::new(215, 3)).unwrap();
        let day_night_bonus = catalog.get(CardKey::new(1630, 2)).unwrap();
        let tolvack = catalog.get(CardKey::new(2690, 1)).unwrap();

        assert!(day_night_ability.night_ability.is_some());
        assert!(day_night_bonus.night_bonus.is_some());
        assert_eq!(tolvack.clan_id, 60);
        assert_eq!(tolvack.clan_name, "Tolvack");
        assert_eq!(catalog.clans().len(), 36);
        assert_eq!(catalog.get_clan(60).unwrap().name, "Tolvack");
    }

    #[test]
    fn returns_none_for_a_missing_key() {
        let catalog = CardCatalog::load(canonical_data_path()).unwrap();

        assert!(catalog.get(CardKey::new(u32::MAX, u8::MAX)).is_none());
    }

    #[test]
    fn rejects_duplicate_card_levels() {
        let row = r#"{
            "id": 123,
            "name": "Natrang",
            "clan_id": 25,
            "clan_name": "Fang Pi Clang",
            "level": 1,
            "level_min": 1,
            "level_max": 3,
            "power": 3,
            "damage": 2,
            "rarity": "c",
            "ability_id": 0,
            "ability": "No Ability",
            "ability_unlock_level": 0,
            "bonus": "Damage +2",
            "bonus_id": 23,
            "release_date": 1139785200
        }"#;
        let json = format!("[{row},{row}]");

        let error = CardCatalog::from_reader(json.as_bytes()).unwrap_err();

        assert!(matches!(
            error,
            CatalogError::DuplicateKey {
                key: CardKey { id: 123, level: 1 }
            }
        ));

        let conflicting_clan = row
            .replace("\"id\": 123", "\"id\": 124")
            .replace("\"bonus\": \"Damage +2\"", "\"bonus\": \"Power +2\"")
            .replace("\"bonus_id\": 23", "\"bonus_id\": 24");
        let json = format!("[{row},{conflicting_clan}]");
        assert!(matches!(
            CardCatalog::from_reader(json.as_bytes()),
            Err(CatalogError::InconsistentClan {
                key: CardKey { id: 124, level: 1 },
                ..
            })
        ));
    }

    #[test]
    fn iteration_is_sorted_by_key() {
        let catalog = CardCatalog::load(canonical_data_path()).unwrap();
        let keys: Vec<_> = catalog.iter().map(|(key, _)| key).collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();

        assert_eq!(keys, sorted);
    }

    #[test]
    fn effective_catalog_applies_and_validates_runtime_overrides() {
        let raw = CardCatalog::load(canonical_data_path()).unwrap();
        let raw_quetzal = raw.get(CardKey::new(1577, 3)).unwrap();
        // The 2026-09-26 card refresh caught up with battle 1131463, so the canonical row
        // already holds the override's `to` and the override is a validated no-op. The
        // synthetic contract test below covers an override that still applies.
        assert_eq!(
            (
                raw_quetzal.power,
                raw_quetzal.damage,
                raw_quetzal.ability.as_str()
            ),
            (7, 4, "Stop Opp. Bonus")
        );

        let effective =
            EffectiveCardCatalog::load(canonical_data_path(), overrides_path()).unwrap();
        let quetzal = effective.get(CardKey::new(1577, 3)).unwrap();
        assert_eq!(
            (
                quetzal.power,
                quetzal.damage,
                quetzal.ability_id,
                quetzal.ability.as_str()
            ),
            (7, 4, 5927, "Stop Opp. Bonus")
        );
        assert_eq!(effective.validated_override_count(), 1);
        assert_eq!(effective.len(), raw.len());
        assert_eq!(
            effective.get(CardKey::new(1577, 4)),
            raw.get(CardKey::new(1577, 4))
        );
        let repeated = EffectiveCardCatalog::load(canonical_data_path(), overrides_path()).unwrap();
        assert_eq!(
            effective.source_fingerprint_fnv1a64(),
            repeated.source_fingerprint_fnv1a64()
        );
        assert_ne!(effective.source_fingerprint_fnv1a64().value(), 0);

        let catalog_bytes = std::fs::read(canonical_data_path()).unwrap();
        let override_bytes = std::fs::read(overrides_path()).unwrap();
        let exact =
            EffectiveCardCatalog::from_readers(catalog_bytes.as_slice(), override_bytes.as_slice())
                .unwrap();
        assert_eq!(
            exact.source_fingerprint_fnv1a64(),
            effective.source_fingerprint_fnv1a64()
        );
        let mut whitespace_changed = override_bytes;
        whitespace_changed.push(b' ');
        let changed = EffectiveCardCatalog::from_readers(
            catalog_bytes.as_slice(),
            whitespace_changed.as_slice(),
        )
        .unwrap();
        assert_ne!(
            changed.source_fingerprint_fnv1a64(),
            effective.source_fingerprint_fnv1a64()
        );
    }

    #[test]
    fn effective_catalog_rejects_stale_and_duplicate_override_contracts() {
        let row = r#"[{
            "id": 1,
            "name": "One",
            "clan_id": 25,
            "clan_name": "Fang Pi Clang",
            "level": 1,
            "level_min": 1,
            "level_max": 1,
            "power": 3,
            "damage": 2,
            "rarity": "c",
            "ability_id": 0,
            "ability": "No Ability",
            "ability_unlock_level": 0,
            "bonus": "Damage +2",
            "bonus_id": 23,
            "release_date": 0
        }]"#;
        let stale = r#"[{
            "id": 1,
            "name": "One",
            "level": 1,
            "from": {"power": 1, "damage": 1, "ability_id": 0, "ability": "No Ability"},
            "to": {"power": 7, "damage": 4, "ability_id": 99, "ability": "Stop Opp. Bonus"},
            "sourceBattle": 99
        }]"#;
        assert!(matches!(
            EffectiveCardCatalog::from_readers(row.as_bytes(), stale.as_bytes()),
            Err(EffectiveCatalogError::UnexpectedOverrideDefinition {
                key: CardKey { id: 1, level: 1 },
                source_battle: 99,
                ..
            })
        ));

        let already_current = row
            .replace("\"power\": 3", "\"power\": 7")
            .replace("\"damage\": 2", "\"damage\": 4")
            .replace("\"ability_id\": 0", "\"ability_id\": 99")
            .replace(
                "\"ability\": \"No Ability\"",
                "\"ability\": \"Stop Opp. Bonus\"",
            );
        let current_contract = r#"[{
            "id": 1,
            "name": "One",
            "level": 1,
            "from": {"power": 3, "damage": 2, "ability_id": 0, "ability": "No Ability"},
            "to": {"power": 7, "damage": 4, "ability_id": 99, "ability": "Stop Opp. Bonus"},
            "sourceBattle": 99
        }]"#;
        let current = EffectiveCardCatalog::from_readers(
            already_current.as_bytes(),
            current_contract.as_bytes(),
        )
        .unwrap();
        assert_eq!(
            (
                current.get(CardKey::new(1, 1)).unwrap().power,
                current.get(CardKey::new(1, 1)).unwrap().damage,
                current.get(CardKey::new(1, 1)).unwrap().ability_id,
                current.get(CardKey::new(1, 1)).unwrap().ability.as_str(),
                current.validated_override_count()
            ),
            (7, 4, 99, "Stop Opp. Bonus", 1)
        );

        // The same contract over a catalog still at `from` applies `to`.
        let applied =
            EffectiveCardCatalog::from_readers(row.as_bytes(), current_contract.as_bytes())
                .unwrap();
        assert_eq!(
            (
                applied.get(CardKey::new(1, 1)).unwrap().power,
                applied.get(CardKey::new(1, 1)).unwrap().damage,
                applied.get(CardKey::new(1, 1)).unwrap().ability_id,
                applied.get(CardKey::new(1, 1)).unwrap().ability.as_str(),
                applied.validated_override_count()
            ),
            (7, 4, 99, "Stop Opp. Bonus", 1)
        );

        let duplicate = r#"[
            {
                "id": 1, "name": "One", "level": 1,
                "from": {"power": 3, "damage": 2, "ability_id": 0, "ability": "No Ability"},
                "to": {"power": 7, "damage": 4, "ability_id": 99, "ability": "Stop Opp. Bonus"},
                "sourceBattle": 99
            },
            {
                "id": 1, "name": "One", "level": 1,
                "from": {"power": 3, "damage": 2, "ability_id": 0, "ability": "No Ability"},
                "to": {"power": 7, "damage": 4, "ability_id": 99, "ability": "Stop Opp. Bonus"},
                "sourceBattle": 100
            }
        ]"#;
        assert!(matches!(
            EffectiveCardCatalog::from_readers(row.as_bytes(), duplicate.as_bytes()),
            Err(EffectiveCatalogError::DuplicateOverride {
                key: CardKey { id: 1, level: 1 }
            })
        ));
    }
}
