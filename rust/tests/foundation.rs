use std::{panic::AssertUnwindSafe, process::Command};

use urban_recreation_rust::{
    ability::ABILITIES,
    card::{BaseCard, Hand},
    game::{Game, GameStatus, PlayerType},
    utils::StackVec4,
};

fn plain_game() -> Game {
    // Distinct clans prevent bonuses, and these four cards have no abilities.
    let hand = Hand::from_names("Bruce", "Alice", "Aurelia", "Brutox");
    Game::new(hand, hand)
}

fn assert_assets() {
    let card = BaseCard::get_name("Gorgorax");
    assert_eq!(card.id, 2581);
    assert_eq!((card.power, card.damage), (6, 8));
    assert_eq!(BaseCard::get_id(card.id).name, "Gorgorax");
    assert!(!ABILITIES[&card.ability_id].modifiers.is_empty());

    let named = Hand::from_names("Gorgorax", "Anagone", "Doela", "Elios");
    let ids = Hand::from_ids(2581, 1983, 1981, 1982);
    for i in 0..4 {
        assert_eq!(named[i].id, ids[i].id);
        assert_eq!(named[i].index, i);
        assert_eq!(format!("{:?}", named[i]), format!("{:?}", ids[i]));
        assert_eq!(named[i].get_ability(), ids[i].get_ability());
    }
}

#[test]
fn assets_load_outside_working_directory() {
    const CHILD: &str = "UR_FOUNDATION_ASSET_CHILD";
    if std::env::var_os(CHILD).is_some() {
        assert_assets();
        return;
    }

    let original_cwd = std::env::current_dir().unwrap();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "urban-recreation-assets-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let result = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "assets_load_outside_working_directory",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .current_dir(&directory)
        .output();
    // The child only reads assets; this directory remains empty.
    std::fs::remove_dir(&directory).unwrap();
    let output = result.unwrap();
    assert!(
        output.status.success(),
        "asset child failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::env::current_dir().unwrap(), original_cwd);
}

#[test]
fn selection_resolves_rounds_and_alternates_first_mover() {
    for flip in [0, 1] {
        let mut game = plain_game();
        game.flip = flip;
        let first = if flip == 0 {
            PlayerType::Player
        } else {
            PlayerType::Opponent
        };
        let second = if flip == 0 {
            PlayerType::Opponent
        } else {
            PlayerType::Player
        };
        assert_eq!(game.get_turn(), first);
        assert_eq!(game.get_first_turn(), first);
        assert!(!game.select(0, 0, false));
        assert_eq!(game.round, 0);
        assert_eq!(game.get_turn(), second);
        assert!(game.select(1, 0, false));
        assert_eq!(game.round, 1);
        assert_eq!(game.get_first_turn(), second);
        assert_eq!(game.get_turn(), second);
        assert!(game.s1.is_none() && game.s2.is_none());
        let (winner_hand, loser_hand) = if flip == 0 {
            (&game.h1, &game.h2)
        } else {
            (&game.h2, &game.h1)
        };
        assert!(winner_hand[0].played && winner_hand[0].won);
        assert!(loser_hand[1].played && !loser_hand[1].won);
        assert!(!winner_hand[1].played && !loser_hand[0].played);
        let lives = if flip == 0 { (12, 9) } else { (9, 12) };
        assert_eq!((game.p1.life, game.p2.life), lives);

        assert!(!game.select(2, 0, false));
        assert_eq!(game.get_turn(), first);
        assert!(game.select(3, 0, false));
        assert_eq!(game.round, 2);
        assert_eq!(game.get_turn(), first);
    }
}

#[test]
fn selection_legality_checks_card_and_total_cost() {
    let mut game = plain_game();
    assert!(game.can_select(0, 0, false));
    assert!(game.can_select(3, 12, false));
    assert!(!game.can_select(4, 0, false));
    assert!(!game.can_select(usize::MAX, 0, false));
    assert!(!game.can_select(0, 13, false));
    assert!(game.can_select(0, 9, true));
    assert!(!game.can_select(0, 10, true));
    game.h1.cards[0].played = true;
    assert!(!game.can_select(0, 0, false));
    game.p1.pillz = 2;
    assert!(!game.can_select(1, 0, true));
    game.p1.pillz = 3;
    assert!(game.can_select(1, 0, true));

    game.flip = 1;
    game.p2.pillz = 1;
    assert!(game.can_select(0, 1, false));
    assert!(!game.can_select(0, 2, false));
    game.h2.cards[1].played = true;
    assert!(!game.can_select(1, 0, false));
}

#[test]
fn completed_game_status_covers_knockouts_and_round_limit() {
    let game = plain_game();
    assert_eq!(game.status(), GameStatus::Playing);
    for (round, life1, life2, expected) in [
        (0, 12, 0, GameStatus::Player),
        (0, 0, 12, GameStatus::Opponent),
        (0, 0, 0, GameStatus::Draw),
        (4, 9, 8, GameStatus::Player),
        (4, 8, 9, GameStatus::Opponent),
        (4, 8, 8, GameStatus::Draw),
    ] {
        let mut state = game;
        state.round = round;
        state.p1.life = life1;
        state.p2.life = life2;
        assert_eq!(state.status(), expected);
    }
}

#[test]
fn copied_games_keep_cards_players_and_events_independent() {
    let mut original = plain_game();
    let toxin = BaseCard::get_name("Zis").to_card(0).get_ability();
    original.events1.add_global(toxin);
    let snapshot = format!("{original:?}");
    let original_events = format!("{:?}", original.events1);
    let mut branch = original;
    branch.select(0, 0, false);
    branch.select(1, 0, false);
    branch.h1.cards[2].power.value += 1;
    branch.h2.cards[3].ability.cancel();
    branch
        .events1
        .add_global(BaseCard::get_name("Timber").to_card(0).get_ability());
    branch.events2.add_global(toxin);
    assert_ne!(format!("{:?}", branch.events1), original_events);
    assert_ne!(format!("{branch:?}"), snapshot);
    assert_eq!(format!("{original:?}"), snapshot);
}

#[test]
fn stackvec_fifth_push_panics_without_overwriting_values() {
    let mut values = StackVec4::default();
    for value in 0..4 {
        values.push(value);
    }
    let before = values;
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| values.push(4)));
    assert!(result.is_err());
    assert_eq!(values, before);
}
