//! Canonical card data keyed by the identity used in captured games.
//!
//! This module deliberately does not construct the legacy engine's [`crate::card::Card`]
//! type. It is the data boundary that future replay and parity work can build on without
//! changing existing game rules.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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

impl CanonicalCard {
    pub const fn key(&self) -> CardKey {
        CardKey::new(self.id, self.level)
    }
}

/// A deterministic index of canonical card rows by card id and level.
#[derive(Clone, Debug, Default)]
pub struct CardCatalog {
    by_key: BTreeMap<CardKey, CanonicalCard>,
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

        for row in rows {
            let key = row.key();
            if by_key.insert(key, row).is_some() {
                return Err(CatalogError::DuplicateKey { key });
            }
        }

        Ok(Self { by_key })
    }

    pub fn get(&self, key: CardKey) -> Option<&CanonicalCard> {
        self.by_key.get(&key)
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
}

#[derive(Debug)]
pub enum CatalogError {
    Open { path: PathBuf, source: io::Error },
    Parse(serde_json::Error),
    DuplicateKey { key: CardKey },
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
        }
    }
}

impl Error for CatalogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Open { source, .. } => Some(source),
            Self::Parse(source) => Some(source),
            Self::DuplicateKey { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CanonicalCard, CardCatalog, CardKey, CatalogError};
    use std::collections::BTreeSet;
    use std::fs::File;
    use std::path::PathBuf;

    fn canonical_data_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/data.json")
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
        let level_three = catalog.get(CardKey::new(123, 3)).unwrap();

        assert_eq!(level_one.name, "Natrang");
        assert_eq!(level_three.name, "Natrang");
        assert_ne!(level_one.power, level_three.power);
        assert_ne!(level_one.damage, level_three.damage);
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
    }

    #[test]
    fn iteration_is_sorted_by_key() {
        let catalog = CardCatalog::load(canonical_data_path()).unwrap();
        let keys: Vec<_> = catalog.iter().map(|(key, _)| key).collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();

        assert_eq!(keys, sorted);
    }
}
