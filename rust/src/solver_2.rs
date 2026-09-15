use std::{collections::HashMap, slice::Iter, time::Instant};

use colored::Colorize;
use lazy_static::lazy_static;

use crate::{
    card::Hand,
    game::{Game, GameStatus, Selection},
    output,
    solver::Solver,
};

/// Tree of results data structures
#[derive(Debug)]
pub enum ResultsTree {
    PlayerWin,
    OpponentWin,
    Draw,
    Map(HashMap<Selection, ResultsTree>),
}

impl ResultsTree {
    pub fn print(&self) {
        self.print_with_depth(0);
    }

    fn print_with_depth(&self, depth: usize) {
        let indent = "  ".repeat(depth);

        match self {
            ResultsTree::PlayerWin => println!("{}Player Wins", indent),
            ResultsTree::OpponentWin => println!("{}Opponent Wins", indent),
            ResultsTree::Draw => println!("{}Draw", indent),
            ResultsTree::Map(map) => {
                if depth == 0 {
                    println!("{{");
                }

                // Sort selections for consistent output
                let mut selections: Vec<_> = map.iter().collect();
                selections.sort_by_key(|(sel, _)| (sel.index, sel.pillz, sel.fury));

                for (selection, tree) in selections {
                    print!(
                        "  {}{} {} {}",
                        indent,
                        selection.index,
                        selection.pillz,
                        if selection.fury {
                            " true".to_string().red()
                        } else {
                            "false".to_string().white()
                        }
                    );
                    match tree {
                        ResultsTree::Map(children) => {
                            let avg = (tree.get_score().1 / 2.0 + 1.0) / 2.0;
                            let avg_fmt = if avg == 1.0 {
                                "100%".to_string().green()
                            } else if avg >= 0.5 {
                                format!("{:.1?}%", avg * 100f32).yellow()
                            } else {
                                format!("{:.1?}%", avg * 100f32).red()
                            };
                            if depth < 1 {
                                println!(" ({}): {{", avg_fmt);
                                tree.print_with_depth(depth + 1);
                                println!("{}  }},", indent);
                            // } else if children.len() == 1 {
                            //     let &(selection, result) = children.iter().next().unwrap();
                            //     print!(
                            //         "  {}{} {} {}",
                            //         indent,
                            //         selection.index,
                            //         selection.pillz,
                            //         if selection.fury {
                            //             " true".to_string().red()
                            //         } else {
                            //             "false".to_string().white()
                            //         }
                            //     );
                            } else {
                                println!(" ({}): {} moves...,", avg_fmt, children.len());
                            }
                        }
                        ResultsTree::Draw => println!(": {}", "Draw,".to_string().bright_black()),
                        ResultsTree::OpponentWin => {
                            println!(": {}", "Opponent Wins,".to_string().red())
                        }
                        ResultsTree::PlayerWin => {
                            println!(": {}", "Player Wins,".to_string().blue())
                        }
                    }
                }

                if depth == 0 {
                    println!("}},");
                }
            }
        }
    }

    /// Get the worst score of the tree and the average win rate.
    /// e.g. If a tree
    fn get_score(&self) -> (i8, f32) {
        match self {
            ResultsTree::PlayerWin => (2, 2.0),
            ResultsTree::OpponentWin => (-2, -2.0),
            ResultsTree::Draw => (1, 1.0),
            ResultsTree::Map(map) => {
                let mut worst_score = 1;
                let mut total_score = 0f32;
                for (_, tree) in map.iter() {
                    let (score, win_rate) = tree.get_score();
                    worst_score = worst_score.min(score);
                    total_score += win_rate;
                }
                (worst_score, total_score / map.len() as f32)
            }
        }
    }

    fn get_best_moves(map: &HashMap<Selection, ResultsTree>) -> (Vec<Selection>, i8, f32) {
        let mut best_moves = Vec::new();
        let mut best_score = 0;
        let mut best_win_rate = 0f32;
        for (selection, tree) in map.iter() {
            let (score, win_rate) = tree.get_score();
            if score > best_score || (score == best_score && win_rate > best_win_rate) {
                best_score = score;
                best_win_rate = win_rate;
                best_moves.clear();
                best_moves.push(*selection);
            } else if score == best_score && win_rate == best_win_rate {
                best_moves.push(*selection);
            }
        }
        let win_percentage = (best_win_rate / 2.0 + 1.0) / 2.0 * 100.0;
        print!("({})", format!("{:.1?}%", win_percentage).green());
        if best_moves.len() == 1 {
            print!(" {}", best_moves[0]);
        } else {
            println!(" {{");
            for selection in best_moves.iter() {
                println!("  {}", selection);
            }
            print!("}}");
        }
        println!();
        (best_moves, best_score, (best_win_rate / 2.0 + 1.0) / 2.0)
    }
}

pub struct Solver2;

impl Solver2 {
    pub fn fill_tree_abab(game: &Game) -> HashMap<Selection, ResultsTree> {
        let _output_guard = output::mute();
        let tree = Solver2::_fill_tree_abab(game);

        tree
    }
    fn _fill_tree_abab(game: &Game) -> HashMap<Selection, ResultsTree> {
        let p2_index = if game.s2.is_some() {
            Some(game.s2.unwrap().index)
        } else {
            None
        };
        let mut result_tree = HashMap::new();

        let pillz1 = game.p1.pillz;
        let pillz2 = game.p2.pillz;

        for i1 in 0..4 {
            if game.h1.cards[i1].played {
                continue;
            }
            for &(p1, f1) in split_shift_range(pillz1) {
                let s1 = Selection {
                    index: i1,
                    pillz: p1,
                    fury: f1,
                };

                let mut tree1 = HashMap::new();

                for i2 in 0..4 {
                    if let Some(index2) = p2_index {
                        if i2 != index2 {
                            continue;
                        }
                    }
                    if game.h2.cards[i2].played {
                        continue;
                    }

                    for &(p2, f2) in split_shift_range(pillz2) {
                        let s2 = Selection {
                            index: i2,
                            pillz: p2,
                            fury: f2,
                        };

                        let mut g = game.clone();
                        g.select_both(s1, s2);

                        match g.status() {
                            GameStatus::Player => {
                                tree1.insert(s2, ResultsTree::PlayerWin);
                            }
                            GameStatus::Opponent => {
                                tree1.insert(s2, ResultsTree::OpponentWin);
                            }
                            GameStatus::Draw => {
                                tree1.insert(s2, ResultsTree::Draw);
                            }
                            GameStatus::Playing => {
                                let tree = Solver2::_fill_tree_abab(&g);
                                tree1.insert(s2, ResultsTree::Map(tree));
                            }
                        }
                    }
                }

                result_tree.insert(s1, ResultsTree::Map(tree1));
            }
        }
        result_tree
    }

    pub fn solve(game: &Game) -> Selection {
        let now = Instant::now();

        let (best_moves, best_score, win_rate, battles) = {
            let _output_guard = output::mute();
            Solver2::_solve(game, 0)
        };

        let elapsed = now.elapsed();
        println!(
            "{} {} /{:.1?}secs ({:.0?}k/s)",
            " Battle Count ".white().on_bright_purple(),
            battles,
            elapsed.as_secs_f32(),
            battles as f32 / elapsed.as_secs_f32() / 1000f32
        );
        match best_score {
            0 => println!("Worst Result | {}", " Draw ".white().on_bright_black()),
            -1 => println!("Worst Result | {}", " Opponent Wins ".white().on_red()),
            1 => println!("Worst Result | {}", " Player Wins ".white().on_blue()),
            _ => unreachable!("Score: {}", best_score),
        }
        for selection in best_moves.iter() {
            println!(
                "({}) {}",
                format!("{:.1?}%", win_rate * 100f32).green(),
                selection
            );
        }
        best_moves[0]
    }

    fn _solve(game: &Game, depth: usize) -> (Vec<Selection>, i8, f32, u32) {
        let p2_index = if game.s2.is_some() {
            Some(game.s2.unwrap().index)
        } else {
            None
        };

        let mut battle_count = 0;

        let mut best_moves = Vec::new();
        let mut best_score = -2;
        let mut best_win_rate = 0f32;

        let pillz1 = game.p1.pillz;
        let pillz2 = game.p2.pillz;

        for i1 in 0..4 {
            if game.h1.cards[i1].played {
                continue;
            }
            for &(p1, f1) in split_shift_range(pillz1) {
                let s1 = Selection {
                    index: i1,
                    pillz: p1,
                    fury: f1,
                };

                let mut worst_score = 1i8;
                let mut total_win_rate = 0f32;
                let mut len = 0;

                // println!("{}{}:", "  ".repeat(depth), format!("{}", s1).on_blue());
                // println!("{}{}:", "  ".repeat(depth), s1);
                // println!("{} {}:", "__".repeat(depth).blue(), s1);

                for i2 in 0..4 {
                    if let Some(index2) = p2_index {
                        if i2 != index2 {
                            continue;
                        }
                    }
                    if game.h2.cards[i2].played {
                        continue;
                    }

                    for &(p2, f2) in split_shift_range(pillz2) {
                        let s2 = Selection {
                            index: i2,
                            pillz: p2,
                            fury: f2,
                        };

                        let mut g = game.clone();
                        g.select_both(s1, s2);

                        battle_count += 1;

                        // print!("{}{}:", "  ".repeat(depth + 1), format!("{}", s2).on_red());
                        // print!("{}{}:", "  ".repeat(depth + 1), s2);
                        // print!("{} {}:", "__".repeat(depth + 1).red(), s2);
                        match g.status() {
                            GameStatus::Player => {
                                total_win_rate += 1.0;
                                // println!("{}", " Player Wins ".white().on_blue());
                            }
                            GameStatus::Opponent => {
                                worst_score = -1;
                                // println!("{}", " Opponent Wins ".white().on_red());
                            }
                            GameStatus::Draw => {
                                worst_score = worst_score.min(0);
                                total_win_rate += 0.5;
                                // println!("{}", " Draw ".white().on_bright_black());
                            }
                            GameStatus::Playing => {
                                // println!();
                                let (_, score, win_rate, battles) = Solver2::_solve(&g, depth + 2);
                                worst_score = worst_score.min(score);
                                total_win_rate += win_rate;
                                battle_count += battles;
                                // println!(
                                //     "{} ({:.1?}%) Worst result {}",
                                //     "__".repeat(depth + 1).red(),
                                //     win_rate * 100.0,
                                //     score
                                // );
                            }
                        }
                        len += 1;
                    }
                }

                // result_tree.insert(s1, ResultsTree::Map(tree1));
                // let (score, win_rate) = ResultsTree::Map(tree1).get_score();
                let score = worst_score;
                let win_rate = total_win_rate / len as f32;
                if score > best_score || (score == best_score && win_rate > best_win_rate) {
                    best_score = score;
                    best_win_rate = win_rate;
                    best_moves.clear();
                    best_moves.push(s1);
                } else if score == best_score && win_rate == best_win_rate {
                    best_moves.push(s1);
                }

                // println!(
                //     "{} ({:.1?}%) Worst result {}",
                //     "__".repeat(depth).blue(),
                //     win_rate * 100.0,
                //     worst_score
                // );
            }
        }

        // (best_moves, best_score, (best_win_rate / 2.0 + 1.0) / 2.0)
        (best_moves, best_score, best_win_rate, battle_count)
    }
}

#[test]
fn test_solver() {
    let h1 = Hand::from_names("Genmaicha", "Orka", "Sando", "Deborah");
    let h2 = Hand::from_names("Nathan", "El Kuzco", "Noon Steevens", "Strygia");

    let mut game = Game::new(h1, h2);
    game.flip = 0;
    // game.flip = 1;

    game.select(1, 3, false); // Orka
    game.select(3, 0, false); // Strygia
                              // game.select(3, 0, false); // Strygia
                              // game.select(1, 3, false); // Orka

    game.select(0, 4, false); // Nathan
    game.select(1, 4, false); // El Kuzco
                              // game.select(0, 2, false); // Genmaicha

    // game.select(2, 0, false); // Sando
    // game.select(2, 3, false); // Noon Steevens

    // game.select(1, 5, false); // El Kuzco

    // let tree = Solver2::fill_tree_abab(&game);

    // let best_moves =
    // ResultsTree::get_best_moves(&tree);
    // println!("{:?}", best_moves);

    // let best =
    Solver2::solve(&game);
    // println!("{}", best);

    let best = Solver::solve(&game);
    println!("{}", best);

    // ResultsTree::Map(tree).print();

    // let selection = best.selection();
    // game.select(selection.index, selection.pillz, selection.fury);

    // let best = Solver::solve(&game);
    // println!("{}", best);
}

static N: u8 = 32;
lazy_static! {
    static ref SPLIT_SHIFT_RANGES: Vec<Vec<(u8, bool)>> = {
        let mut ranges = Vec::with_capacity(N as usize);
        for n in 0..N {
            let mut range = Vec::new();

            range.push((n, false));

            if n < 3 {
                for i in 0..n {
                    range.push((i, false));
                }
            } else {
                range.push((n - 3, false));

                for i in 0..n - 3 {
                    range.push((i, false));
                }

                range.push((n - 2, false));
                range.push((n - 1, false));

                range.push((n - 3, true));
                for i in 0..n - 3 {
                    range.push((i, true));
                }
            }

            ranges.push(range);
        }

        ranges
    };
}

#[inline]
fn split_shift_range(n: u8) -> Iter<'static, (u8, bool)> {
    SPLIT_SHIFT_RANGES[n as usize].iter()
}
