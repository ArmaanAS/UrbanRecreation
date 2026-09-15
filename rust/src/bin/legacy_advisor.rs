use std::{
    env,
    io::{self, Result},
    sync::{Arc, Mutex},
    thread,
};

use actix_web::web::Data;
use rayon::ThreadPoolBuilder;
use urban_recreation_rust::{
    card::Hand,
    game::{Game, GameStatus, PlayerType, Selection},
    server,
    solver::{SelectionResult, Solver},
    solver_2::Solver2,
};

#[allow(unreachable_code)]
#[actix_web::main]
async fn main() -> Result<()> {
    ThreadPoolBuilder::new()
        .num_threads(4)
        .build_global()
        .unwrap();
    let args: Vec<String> = env::args().collect();
    let h1: Hand;
    let h2: Hand;
    let mut flip = 0u8;
    let game: Arc<Mutex<Option<Game>>> = Arc::new(Mutex::new(None));
    if args.len() >= 9 {
        h1 = Hand::from_names(
            args[1].as_str(),
            args[2].as_str(),
            args[3].as_str(),
            args[4].as_str(),
        );
        h2 = Hand::from_names(
            args[5].as_str(),
            args[6].as_str(),
            args[7].as_str(),
            args[8].as_str(),
        );
        if args.len() == 10 {
            flip = 1;
        }
        let mut g = Game::new(h1, h2);
        g.flip = flip;
        g.print_status();

        if flip == 0 {
            Solver::middle(&g);
        }

        println!("{} turn", g.get_turn_name());

        *game.lock().unwrap() = Some(g);
    }

    let game_clone = game.clone();

    thread::spawn(move || {
        for line in io::stdin().lines() {
            let mut input = line.unwrap();

            let mut game_lock = game.lock().unwrap();
            if game_lock.is_none() {
                continue;
            }
            let game = game_lock.as_mut().unwrap();

            if input.as_str() == "cancel" {
                game.clear_selection();
                game.print_status();
                continue;
            } else if input.starts_with("x ") {
                input = input[2..].to_string();
                game.clear_selection();
            }

            let selected = Selection::parse(input);
            if selected.is_none() {
                continue;
            }

            let Selection { index, pillz, fury } = selected.unwrap();

            if !game.can_select(index, pillz, fury) {
                continue;
            }

            let battled = game.select(index, pillz, fury);
            let game = game.clone();

            if battled {
                game.print_status();
            }
            if game.status() != GameStatus::Playing {
                continue;
            }

            let turn = game.get_turn();

            if game.round != 0 {
                let game_clone = game.clone();
                thread::spawn(move || {
                    Solver2::solve(&game_clone);
                    println!("{} turn", game.get_turn_name());
                });

                let best = Solver::solve(&game);

                match (best, turn) {
                    (SelectionResult::Player(_), PlayerType::Opponent)
                    | (SelectionResult::Opponent(_), PlayerType::Player) => {
                        Solver::middle(&game);
                    }
                    (_, _) => println!("{}", best),
                }
            }

            println!("{} turn", game.get_turn_name());
        }
    });

    server::serve(Data::new(Arc::clone(&game_clone))).await?;

    Ok(())
}
