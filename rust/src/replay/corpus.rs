use super::adapter::{adapt_capture, AdapterError, ReplayClassification, SkippedReplay};
use super::capture::CapturedGame;
use super::model::ReplayCaseV1;
use crate::catalog::{CardCatalog, CatalogError};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct ReplayCorpus {
    pub ready: Vec<ReplayCaseV1>,
    pub skipped: Vec<SkippedReplay>,
    pub errors: Vec<CorpusEntryError>,
}

impl ReplayCorpus {
    pub fn len(&self) -> usize {
        self.ready.len() + self.skipped.len() + self.errors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug)]
pub struct CorpusEntryError {
    pub path: PathBuf,
    pub battle_id: Option<u64>,
    pub kind: CorpusErrorKind,
}

#[derive(Debug)]
pub enum CorpusErrorKind {
    Open(io::Error),
    Parse(serde_json::Error),
    Adapt(AdapterError),
    DuplicateBattleId { battle_id: u64, paths: Vec<PathBuf> },
}

impl fmt::Display for CorpusEntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: ", self.path.display())?;
        match &self.kind {
            CorpusErrorKind::Open(source) => write!(formatter, "failed to open capture: {source}"),
            CorpusErrorKind::Parse(source) => {
                write!(formatter, "failed to parse capture: {source}")
            }
            CorpusErrorKind::Adapt(source) => source.fmt(formatter),
            CorpusErrorKind::DuplicateBattleId { battle_id, paths } => write!(
                formatter,
                "duplicate battle id {battle_id} occurs in {}",
                paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl Error for CorpusEntryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            CorpusErrorKind::Open(source) => Some(source),
            CorpusErrorKind::Parse(source) => Some(source),
            CorpusErrorKind::Adapt(source) => Some(source),
            CorpusErrorKind::DuplicateBattleId { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum CorpusLoadError {
    Catalog(CatalogError),
    ReadDirectory { path: PathBuf, source: io::Error },
}

impl fmt::Display for CorpusLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalog(source) => write!(formatter, "failed to load card catalog: {source}"),
            Self::ReadDirectory { path, source } => {
                write!(
                    formatter,
                    "failed to read capture directory {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl Error for CorpusLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Catalog(source) => Some(source),
            Self::ReadDirectory { source, .. } => Some(source),
        }
    }
}

/// Loads every JSON capture with an explicitly supplied canonical card catalog.
///
/// A bad directory or catalog prevents loading. Individual capture failures are retained
/// in `ReplayCorpus::errors`, allowing callers to see the complete, sorted error set.
pub fn load_corpus(
    games_directory: impl AsRef<Path>,
    catalog_path: impl AsRef<Path>,
) -> Result<ReplayCorpus, CorpusLoadError> {
    let games_directory = games_directory.as_ref();
    let catalog = CardCatalog::load(catalog_path).map_err(CorpusLoadError::Catalog)?;
    let directory =
        fs::read_dir(games_directory).map_err(|source| CorpusLoadError::ReadDirectory {
            path: games_directory.to_path_buf(),
            source,
        })?;
    let mut paths = Vec::new();
    for entry in directory {
        let entry = entry.map_err(|source| CorpusLoadError::ReadDirectory {
            path: games_directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            paths.push(path);
        }
    }
    paths.sort_by(|left, right| compare_capture_paths(left, right));

    let mut corpus = ReplayCorpus::default();
    let mut parsed = Vec::new();
    for path in paths {
        let battle_id = battle_id_from_path(&path);
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(source) => {
                corpus.errors.push(CorpusEntryError {
                    path,
                    battle_id,
                    kind: CorpusErrorKind::Open(source),
                });
                continue;
            }
        };
        let capture = match CapturedGame::from_reader(file) {
            Ok(capture) => capture,
            Err(source) => {
                corpus.errors.push(CorpusEntryError {
                    path,
                    battle_id,
                    kind: CorpusErrorKind::Parse(source),
                });
                continue;
            }
        };
        parsed.push((path, capture));
    }

    let mut paths_by_battle_id: BTreeMap<u64, Vec<PathBuf>> = BTreeMap::new();
    for (path, capture) in &parsed {
        paths_by_battle_id
            .entry(capture.battle_id())
            .or_default()
            .push(path.clone());
    }

    for (path, capture) in parsed {
        let battle_id = capture.battle_id();
        let duplicate_paths = &paths_by_battle_id[&battle_id];
        if duplicate_paths.len() > 1 {
            corpus.errors.push(CorpusEntryError {
                path,
                battle_id: Some(battle_id),
                kind: CorpusErrorKind::DuplicateBattleId {
                    battle_id,
                    paths: duplicate_paths.clone(),
                },
            });
            continue;
        }

        match adapt_capture(capture, &catalog) {
            Ok(ReplayClassification::Ready(replay)) => corpus.ready.push(*replay),
            Ok(ReplayClassification::Skipped(skipped)) => corpus.skipped.push(skipped),
            Err(source) => corpus.errors.push(CorpusEntryError {
                path,
                battle_id: Some(source.battle_id),
                kind: CorpusErrorKind::Adapt(source),
            }),
        }
    }

    corpus.ready.sort_by_key(|replay| replay.metadata.battle_id);
    corpus.skipped.sort_by_key(|skipped| skipped.battle_id);
    corpus
        .errors
        .sort_by(|left, right| compare_capture_paths(&left.path, &right.path));

    Ok(corpus)
}

fn battle_id_from_path(path: &Path) -> Option<u64> {
    path.file_stem()?.to_str()?.parse().ok()
}

fn compare_capture_paths(left: &Path, right: &Path) -> Ordering {
    let numeric_order = match (battle_id_from_path(left), battle_id_from_path(right)) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    };
    numeric_order.then_with(|| left.cmp(right))
}

#[cfg(test)]
mod tests {
    use super::{load_corpus, CorpusErrorKind};
    use crate::catalog::CardCatalog;
    use crate::replay::adapter::ReplaySkipReason;
    use crate::replay::execute::BaseRulesReplay;
    use crate::replay::model::{ReplayCaseV1, SourceStatus, REPLAY_SCHEMA_VERSION};
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root_path(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(path)
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "urban-recreation-replay-corpus-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn collects_every_file_error_in_numeric_order() {
        let directory = TestDirectory::new();
        fs::write(directory.0.join("20.json"), b"not json").unwrap();
        fs::write(directory.0.join("3.json"), b"also not json").unwrap();

        let corpus = load_corpus(&directory.0, root_path("data/data.json")).unwrap();

        assert_eq!(corpus.len(), 2);
        assert_eq!(
            corpus
                .errors
                .iter()
                .map(|error| error.battle_id)
                .collect::<Vec<_>>(),
            [Some(3), Some(20)]
        );
    }

    #[test]
    fn rejects_every_copy_of_a_duplicate_battle_id_deterministically() {
        let directory = TestDirectory::new();
        let original = fs::read(root_path("captures/games/866431.json")).unwrap();
        fs::write(directory.0.join("866431.json"), &original).unwrap();
        fs::write(directory.0.join("0866431.json"), &original).unwrap();

        let mut changed: serde_json::Value = serde_json::from_slice(&original).unwrap();
        changed["snapshots"] = serde_json::json!(changed["snapshots"].as_u64().unwrap() + 1);
        fs::write(
            directory.0.join("duplicate.json"),
            serde_json::to_vec(&changed).unwrap(),
        )
        .unwrap();

        let corpus = load_corpus(&directory.0, root_path("data/data.json")).unwrap();

        assert_eq!(corpus.len(), 3);
        assert!(corpus.ready.is_empty());
        assert!(corpus.skipped.is_empty());
        assert!(corpus.errors.iter().all(|error| matches!(
            error.kind,
            CorpusErrorKind::DuplicateBattleId {
                battle_id: 866431,
                ..
            }
        )));
        assert_eq!(
            corpus
                .errors
                .iter()
                .map(|error| error.path.file_name().unwrap().to_str().unwrap())
                .collect::<Vec<_>>(),
            ["0866431.json", "866431.json", "duplicate.json"]
        );
    }

    #[test]
    fn classifies_the_complete_capture_corpus() {
        let games_path = root_path("captures/games");
        let catalog_path = root_path("data/data.json");
        let corpus = load_corpus(&games_path, &catalog_path).unwrap();
        let file_count = fs::read_dir(&games_path)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
            })
            .count();

        assert!(file_count >= 328);
        assert_eq!(corpus.len(), file_count);
        assert!(
            corpus.errors.is_empty(),
            "capture errors: {:#?}",
            corpus.errors
        );
        assert_eq!(corpus.skipped.len(), 7);
        assert!(corpus.ready.len() >= 352);

        let skipped_ids: Vec<_> = corpus
            .skipped
            .iter()
            .map(|skipped| skipped.battle_id)
            .collect();
        assert_eq!(
            skipped_ids,
            [830285, 869944, 957643, 1009234, 1024388, 1092729, 1145959]
        );
        assert_eq!(corpus.skipped[0].reason, ReplaySkipReason::NoReplayTestcase);
        assert!(corpus.skipped[1..]
            .iter()
            .all(|skipped| skipped.reason == ReplaySkipReason::InProgress));

        let ready_ids: Vec<_> = corpus
            .ready
            .iter()
            .map(|replay| replay.metadata.battle_id)
            .collect();
        assert!(ready_ids.windows(2).all(|ids| ids[0] < ids[1]));
        assert!(corpus
            .ready
            .iter()
            .any(|replay| replay.metadata.source_status == SourceStatus::Timeout));
        assert!(corpus
            .ready
            .iter()
            .any(|replay| replay.metadata.source_status == SourceStatus::Left));
        assert!(
            corpus
                .ready
                .iter()
                .map(|replay| replay.rounds.len())
                .sum::<usize>()
                >= 1_102
        );

        let catalog = CardCatalog::load(catalog_path).unwrap();
        for replay in &corpus.ready {
            BaseRulesReplay::new(replay.clone(), &catalog).unwrap_or_else(|error| {
                panic!(
                    "ready battle {} failed current-engine replay validation: {error}",
                    replay.metadata.battle_id
                )
            });
            assert_eq!(replay.schema_version, REPLAY_SCHEMA_VERSION);
            let mut played = BTreeSet::new();
            for player in &replay.players {
                for card in &player.hand {
                    assert!(catalog.get(card.key).is_some());
                }
            }
            for round in &replay.rounds {
                assert!(played.insert((round.plays[0].engine_player, round.plays[0].hand_index)));
                assert!(played.insert((round.plays[1].engine_player, round.plays[1].hand_index)));
            }
        }

        let json = serde_json::to_vec(&corpus.ready[0]).unwrap();
        let round_trip: ReplayCaseV1 = serde_json::from_slice(&json).unwrap();
        assert_eq!(round_trip, corpus.ready[0]);

        let mut unsupported: serde_json::Value = serde_json::from_slice(&json).unwrap();
        unsupported["schema_version"] = serde_json::json!(2);
        let error = serde_json::from_value::<ReplayCaseV1>(unsupported).unwrap_err();
        assert!(error
            .to_string()
            .contains("unsupported replay schema version 2"));
    }
}
