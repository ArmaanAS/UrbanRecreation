#[cfg(test)]
mod tests {
    use crate::{
        card::Hand,
        game::{Game, PlayerType},
    };
    use serde::Deserialize;
    use std::{
        fmt::Write,
        fs,
        panic::{catch_unwind, AssertUnwindSafe},
    };

    #[derive(Deserialize)]
    struct Move {
        s1: (usize, u8, bool),
        s2: (usize, u8, bool),
        p1life: u8,
        p2life: u8,
        p1pillz: u8,
        p2pillz: u8,
    }
    #[derive(Deserialize)]
    struct Testcase {
        cards: [String; 8],
        flip: bool,
        life: u8,
        pillz: u8,
        moves: Vec<Move>,
    }

    /// Archaeology of the imported engine, not evidence of correct game rules.
    #[test]
    #[ignore = "historical release baseline; run explicitly with --release and one test thread"]
    fn historical_release_baseline() {
        assert!(
            !cfg!(debug_assertions),
            "the archived baseline uses release overflow behavior"
        );
        let _guard = crate::output::mute();
        let cases: Vec<Testcase> = serde_json::from_slice(
            &fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/testcases10000.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let mut rows = String::from("case,round,p1life,p2life,p1pillz,p2pillz,status,p1index,p1id,p1power,p1damage,p1attack,p1won,p1played,p2index,p2id,p2power,p2damage,p2attack,p2won,p2played\n");
        let mut failed = Vec::new();
        let mut mismatched = Vec::new();
        let mut panicked = Vec::new();
        let mut resolved_rounds = 0;
        for (i, t) in cases.iter().enumerate() {
            let mut mismatch = false;
            let result = catch_unwind(AssertUnwindSafe(|| {
                let h1 = Hand::from_names(&t.cards[0], &t.cards[1], &t.cards[2], &t.cards[3]);
                let h2 = Hand::from_names(&t.cards[4], &t.cards[5], &t.cards[6], &t.cards[7]);
                let mut game = Game::new(h1, h2);
                game.flip = t.flip as u8;
                game.p1.life = t.life;
                game.p2.life = t.life;
                game.p1.pillz = t.pillz;
                game.p2.pillz = t.pillz;
                for (round, m) in t.moves.iter().enumerate() {
                    let (p1index, p2index) = if game.get_turn() == PlayerType::Player {
                        (m.s1.0, m.s2.0)
                    } else {
                        (m.s2.0, m.s1.0)
                    };
                    game.select(m.s1.0, m.s1.1, m.s1.2);
                    game.select(m.s2.0, m.s2.1, m.s2.2);
                    let a = &game.h1[p1index];
                    let b = &game.h2[p2index];
                    writeln!(&mut rows,
                        "{i},{round},{},{},{},{},{:?},{p1index},{},{},{},{},{},{},{p2index},{},{},{},{},{},{}",
                        game.p1.life, game.p2.life, game.p1.pillz, game.p2.pillz, game.status(),
                        a.id, a.power.value, a.damage.value, a.attack.value, a.won, a.played,
                        b.id, b.power.value, b.damage.value, b.attack.value, b.won, b.played,
                    ).unwrap();
                    resolved_rounds += 1;
                    let actual = (game.p1.life, game.p2.life, game.p1.pillz, game.p2.pillz);
                    let expected = (m.p1life, m.p2life, m.p1pillz, m.p2pillz);
                    if actual != expected {
                        mismatch = true;
                    }
                }
            }));
            let panic = result.is_err();
            if mismatch {
                mismatched.push(i);
            }
            if panic {
                panicked.push(i);
            }
            if mismatch || panic {
                failed.push(i);
            }
        }
        let digest = rows
            .as_bytes()
            .iter()
            .fold(0xcbf29ce484222325u64, |hash, b| {
                (hash ^ u64::from(*b)).wrapping_mul(0x100000001b3)
            });
        let report = serde_json::json!({
            "total": cases.len(), "passed": cases.len() - failed.len(),
            "failed": failed.len(), "mismatched": mismatched.len(), "panicked": panicked.len(),
            "resolved_rounds": resolved_rounds, "round_rows_fnv1a64": format!("{digest:016x}"),
            "failed_indexes": failed, "mismatched_indexes": mismatched, "panic_indexes": panicked,
        });
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/historical-release-baseline.json"
        ))
        .unwrap();
        assert_eq!(report, expected, "historical engine behavior changed");
        println!("Historical baseline: 10000 cases, 939 failing cases, round digest {digest:016x}");
    }
}
