use std::fs::File;
use std::path::PathBuf;

use urban_recreation_rust::catalog::CardCatalog;
use urban_recreation_rust::engine::{MatchStatus, PlayerId};
use urban_recreation_rust::replay::{
    adapt_capture, BaseRulesReplay, BaseRulesReplayError, CapturedGame, EnginePlayer,
    ReplayClassification,
};

const BASE_RULES_PREFIX_FIXTURES: &[(u64, usize)] = &[
    (874520, 1),
    (876464, 1),
    (878056, 1),
    (878120, 1),
    (924257, 1),
    (924890, 1),
    (925087, 1),
    (926367, 1),
    (943231, 1),
    (964352, 1),
    (1023946, 1),
    (1065231, 2),
    (1065673, 1),
    (1069506, 2),
    (1072885, 1),
    (1081879, 1),
    (1087884, 1),
    (1093399, 1),
];

fn root_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(path)
}

fn catalog() -> CardCatalog {
    CardCatalog::load(root_path("data/data.json")).unwrap()
}

fn replay(id: u64, catalog: &CardCatalog) -> urban_recreation_rust::replay::ReplayCaseV1 {
    let capture = CapturedGame::from_reader(
        File::open(root_path(&format!("captures/games/{id}.json"))).unwrap(),
    )
    .unwrap();
    let ReplayClassification::Ready(replay) = adapt_capture(capture, catalog).unwrap() else {
        panic!("battle {id} was not replay-ready");
    };
    *replay
}

#[test]
fn battle_1065231_runs_end_to_end_under_effects_disabled_base_rules() {
    let catalog = catalog();
    let replay = BaseRulesReplay::new(replay(1065231, &catalog), &catalog).unwrap();
    let report = replay.execute_effects_disabled().unwrap();

    assert!(!report.effects_enabled);
    assert_eq!(report.rounds.len(), 2);
    assert_eq!(report.rounds[0].cards[PlayerId::P1].attack, 49);
    assert_eq!(report.rounds[0].cards[PlayerId::P2].attack, 18);
    assert_eq!(report.rounds[0].players[PlayerId::P1].life, 12);
    assert_eq!(report.rounds[0].players[PlayerId::P2].life, 7);
    assert_eq!(report.rounds[1].first_mover, PlayerId::P2);
    assert_eq!(report.rounds[1].cards[PlayerId::P1].damage, 7);
    assert_eq!(report.rounds[1].cards[PlayerId::P1].attack, 24);
    assert_eq!(report.rounds[1].cards[PlayerId::P2].attack, 14);
    assert_eq!(report.final_position.players[PlayerId::P1].pillz, 0);
    assert_eq!(report.final_position.players[PlayerId::P2].pillz, 9);
    assert_eq!(report.final_position.status, MatchStatus::Won(PlayerId::P1));

    assert!(replay.match_spec().night);
    assert_eq!(replay.match_spec().battle_rule_id, 10);
    assert_eq!(
        replay.match_spec().players[PlayerId::P1].hand[0].clan_id,
        41
    );
}

#[test]
fn audited_effects_disabled_prefix_lengths_are_fixed_fixtures() {
    let catalog = catalog();
    let mut asserted_rounds = 0;
    for &(battle_id, prefix_len) in BASE_RULES_PREFIX_FIXTURES {
        let replay = BaseRulesReplay::new(replay(battle_id, &catalog), &catalog).unwrap();
        let report = replay
            .execute_effects_disabled_prefix(prefix_len)
            .unwrap_or_else(|error| panic!("fixed base-rules fixture {battle_id}: {error}"));
        assert_eq!(report.rounds.len(), prefix_len);
        asserted_rounds += report.rounds.len();
    }
    assert_eq!(asserted_rounds, 20);
}

#[test]
fn constructor_revalidates_public_replay_invariants() {
    let catalog = catalog();
    let original = replay(1065231, &catalog);

    let mut bad_schema = original.clone();
    bad_schema.schema_version = 2;
    let error = BaseRulesReplay::new(bad_schema, &catalog).unwrap_err();
    assert_eq!(error.field, "schema_version");

    let mut bad_identity = original.clone();
    bad_identity.players[0].engine_player = EnginePlayer::P2;
    let error = BaseRulesReplay::new(bad_identity, &catalog).unwrap_err();
    assert_eq!(error.field, "players[0].engine_player");

    let mut bad_first_round = original.clone();
    bad_first_round.rounds[0].first_mover = EnginePlayer::P2;
    let error = BaseRulesReplay::new(bad_first_round, &catalog).unwrap_err();
    assert_eq!(error.field, "rounds[0].first_mover");

    let mut reused = original;
    reused.rounds[1].plays[1].hand_index = reused.rounds[0].plays[0].hand_index;
    reused.rounds[1].plays[1].card = reused.rounds[0].plays[0].card;
    let error = BaseRulesReplay::new(reused, &catalog).unwrap_err();
    assert!(error.detail.contains("already played"));
}

#[test]
fn expected_outputs_are_assertions_and_never_engine_inputs() {
    let catalog = catalog();
    let mut source = replay(1065231, &catalog);
    source.rounds[0].expected_player_states[0].life = 999;
    let replay = BaseRulesReplay::new(source, &catalog).unwrap();

    let error = replay.execute_effects_disabled().unwrap_err();
    let BaseRulesReplayError::Mismatch {
        context,
        field,
        expected,
        actual,
    } = &error
    else {
        panic!("expected assertion mismatch, found {error:?}");
    };
    assert_eq!(field, "players.P1.life");
    assert_eq!(expected, "999");
    assert_eq!(actual, "12");
    assert_eq!(context.battle_id, 1065231);
    assert_eq!(context.round, 0);
    let diagnostic = error.to_string();
    assert!(diagnostic.contains("559@5"));
    assert!(diagnostic.contains("1051@2"));
    assert!(diagnostic.contains("hand_index: 0"));
}

#[test]
fn expected_attack_schema_accepts_values_beyond_u16() {
    let catalog = catalog();
    let mut source = replay(1065231, &catalog);
    source.rounds[0].expected_card_results[0]
        .as_mut()
        .unwrap()
        .attack = 82_000;
    let json = serde_json::to_vec(&source).unwrap();
    let decoded: urban_recreation_rust::replay::ReplayCaseV1 =
        serde_json::from_slice(&json).unwrap();
    assert_eq!(
        decoded.rounds[0].expected_card_results[0].unwrap().attack,
        82_000
    );
}
